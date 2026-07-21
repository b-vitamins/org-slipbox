;;; org-slipbox-glossary.el --- Glossary commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.16.0
;; Package-Requires: ((emacs "29.1") (jsonrpc "1.0.27"))
;; Keywords: outlines, files, convenience

;; This file is not part of GNU Emacs.

;; org-slipbox is free software: you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.
;;
;; org-slipbox is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.
;;
;; You should have received a copy of the GNU General Public License
;; along with org-slipbox.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; Glossary capture and lookup commands for `org-slipbox'.
;;
;; A glossary term is an ordinary Org file node carrying the `#+glossary: t'
;; marker: the headword is its `#+title', synonyms are `ROAM_ALIASES', and the
;; definition is the file body.  This module captures terms through the shared
;; capture path, looks them up with their definition shown inline, and peeks the
;; definition of the term at point.

;;; Code:

(require 'seq)
(require 'subr-x)
(require 'org-slipbox-capture)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-glossary-read-limit 200
  "Maximum number of indexed glossary terms to request for completion."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-glossary-capture-templates
  '(("g" "glossary term" plain "${body}"
     :target (file+head
              "${slug}.org"
              "#+title: ${title}\n#+glossary: t\n")
     :title "${title}"))
  "Capture templates used by `org-slipbox-glossary-define'.
These templates use the same syntax as `org-slipbox-capture-templates'.
The default marks the file node with `#+glossary: t' and inserts the
definition, interpolated from `${body}', as the term body."
  :type 'sexp
  :group 'org-slipbox)

(defcustom org-slipbox-glossary-annotation-function
  #'org-slipbox-glossary-read--annotation
  "Function used to annotate `org-slipbox-glossary-read' candidates.
The function receives one term NODE plist and must return a string."
  :type 'function
  :group 'org-slipbox)

(defvar org-slipbox-glossary-history nil
  "Minibuffer history for `org-slipbox-glossary-read'.")

;;;###autoload
(defun org-slipbox-glossary-define (headword &optional definition status)
  "Define a glossary term for HEADWORD and visit it.
DEFINITION seeds the term body; when nil, the definition is written in
the capture draft.  STATUS is the confirmation status, a symbol `stub'
or `confirmed' (or the matching string), defaulting to `stub'.
An existing term matching HEADWORD is visited instead of duplicated."
  (interactive
   (list (read-string "Headword: ")
         nil
         (intern
          (completing-read "Status: " '("stub" "confirmed") nil t nil nil "stub"))))
  (setq headword (string-trim headword))
  (when (string-empty-p headword)
    (user-error "Headword must not be empty"))
  (let ((existing (org-slipbox--live-node-or-nil
                   (org-slipbox-node-from-title-or-alias headword t))))
    (if existing
        (progn
          (org-slipbox--visit-node existing)
          existing)
      (org-slipbox--capture-node
       headword
       (org-slipbox--read-capture-template org-slipbox-glossary-capture-templates)
       nil
       (list :body (or definition ""))
       (list :default-finalize
             (org-slipbox-glossary--make-capture-finalizer
              (org-slipbox-glossary--status-string (or status 'stub))))))))

;;;###autoload
(defun org-slipbox-glossary-find (&optional initial-input prompt)
  "Find and visit a glossary term.
INITIAL-INPUT seeds the minibuffer.  PROMPT defaults to \"Term: \"."
  (interactive)
  (let ((node (org-slipbox-glossary-read initial-input prompt)))
    (when node
      (org-slipbox--visit-node node))))

;;;###autoload
(defun org-slipbox-glossary-peek (&optional node)
  "Show the definition for the glossary term at point.
NODE overrides the term resolved from context.  When point is not inside
a term, read one through completion."
  (interactive)
  (let ((node (or node
                  (org-slipbox-glossary--term-at-point)
                  (org-slipbox-glossary-read nil "Peek term: "))))
    (unless node
      (user-error "No glossary term to peek"))
    (org-slipbox-glossary--display-definition
     node
     (org-slipbox-glossary--definition node))))

(defun org-slipbox-glossary-read (&optional initial-input prompt)
  "Read and return an indexed glossary term.
INITIAL-INPUT seeds the minibuffer.  PROMPT defaults to \"Term: \"."
  (let* ((prompt (or prompt "Term: "))
         completions
         (collection
          (lambda (string pred action)
            (if (eq action 'metadata)
                `(metadata
                  (annotation-function
                   . ,(lambda (candidate)
                        (org-slipbox-glossary-completion-annotation candidate)))
                  (category . org-slipbox-glossary))
              (setq completions
                    (org-slipbox-glossary-completion-candidates string))
              (complete-with-action action completions string pred))))
         (selection
          (completing-read
           prompt
           collection
           nil
           t
           initial-input
           'org-slipbox-glossary-history))
         (node (cdr (assoc selection completions))))
    (or node
        (cdr (assoc selection
                    (org-slipbox-glossary-completion-candidates selection))))))

(defun org-slipbox-glossary-completion-candidates (query)
  "Return formatted glossary completion candidates for QUERY."
  (let* ((response (org-slipbox-rpc-search-glossary
                    query org-slipbox-glossary-read-limit))
         (terms (org-slipbox--plist-sequence (plist-get response :terms))))
    (mapcar #'org-slipbox-glossary--completion-candidate terms)))

(defun org-slipbox-glossary-read--annotation (node)
  "Return the default completion annotation for term NODE."
  (let ((definition (org-slipbox-glossary--definition node)))
    (if (and definition (not (string-empty-p definition)))
        (format "  %s" (org-slipbox-glossary--one-line definition))
      "")))

(defun org-slipbox-glossary--completion-candidate (node)
  "Return a display-to-node completion pair for term NODE."
  (let* ((visible
          (propertize (or (plist-get node :title) "")
                      'org-slipbox-glossary-node node))
         (hidden
          (propertize
           (or (plist-get node :node_key)
               (plist-get node :explicit_id)
               (format "%s:%s"
                       (plist-get node :file_path)
                       (plist-get node :line)))
           'invisible t)))
    (cons (concat visible hidden) node)))

(defun org-slipbox-glossary-completion-annotation (candidate)
  "Return the annotation string for glossary completion CANDIDATE."
  (if-let ((node (get-text-property 0 'org-slipbox-glossary-node candidate)))
      (funcall org-slipbox-glossary-annotation-function node)
    ""))

(defun org-slipbox-glossary--make-capture-finalizer (status)
  "Return a capture finalize function recording STATUS then visiting the node."
  (lambda (node _session)
    (when-let ((node-key (plist-get node :node_key)))
      (org-slipbox-rpc-mark-glossary-term node-key status))
    (org-slipbox--visit-node node)))

(defun org-slipbox-glossary--status-string (status)
  "Return the glossary status STATUS as a canonical string."
  (pcase status
    ('stub "stub")
    ('confirmed "confirmed")
    ((pred stringp) status)
    (_ "stub")))

(defun org-slipbox-glossary--term-at-point ()
  "Return the glossary term node at point, or nil."
  (let ((node (ignore-errors (org-slipbox-node-at-point))))
    (when (eq (plist-get node :glossary) t)
      node)))

(defun org-slipbox-glossary--definition (node)
  "Return the definition body text for glossary NODE, or nil."
  (when-let ((file (ignore-errors (org-slipbox-node-file node))))
    (when (file-readable-p file)
      (with-temp-buffer
        (insert-file-contents file)
        (org-slipbox-glossary--buffer-definition)))))

(defun org-slipbox-glossary--buffer-definition ()
  "Return the leading definition paragraph in the current buffer, or nil.
Skips file keywords and a leading property drawer, then returns the
first non-empty paragraph as trimmed text."
  (goto-char (point-min))
  (let ((case-fold-search t))
    (catch 'done
      (while (not (eobp))
        (cond
         ((looking-at-p "[ \t]*#\\+") (forward-line 1))
         ((looking-at-p "[ \t]*:PROPERTIES:[ \t]*$")
          (if (re-search-forward "^[ \t]*:END:[ \t]*$" nil t)
              (forward-line 1)
            (throw 'done nil)))
         ((looking-at-p "[ \t]*$") (forward-line 1))
         (t (throw 'done nil)))))
    (let ((start (point)))
      (while (and (not (eobp))
                  (not (looking-at-p "[ \t]*$")))
        (forward-line 1))
      (let ((text (string-trim (buffer-substring-no-properties start (point)))))
        (unless (string-empty-p text)
          text)))))

(defun org-slipbox-glossary--one-line (text)
  "Return TEXT collapsed to a single whitespace-separated line."
  (replace-regexp-in-string "[ \t\n]+" " " (string-trim text)))

(defun org-slipbox-glossary--display-definition (node definition)
  "Display DEFINITION for term NODE."
  (let ((title (or (plist-get node :title) "")))
    (if (and definition (not (string-empty-p definition)))
        (message "%s: %s" title (org-slipbox-glossary--one-line definition))
      (message "%s: (no definition)" title))))

(provide 'org-slipbox-glossary)

;;; org-slipbox-glossary.el ends here
