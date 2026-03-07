;;; org-slipbox-capture.el --- Capture commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: org-slipbox contributors <maintainers@example.invalid>
;; Maintainer: org-slipbox contributors <maintainers@example.invalid>
;; Version: 0.0.0
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

;; Capture commands and template expansion helpers for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-capture-templates
  '(("d" "default" :path "${slug}.org" :title "${title}"))
  "Capture templates for `org-slipbox-capture'.
Each template is a list of the form
\(KEY DESCRIPTION [:path STRING] [:title STRING] [:target SPEC]\).

When present, SPEC may be one of:
- (file PATH)
- (file+head PATH HEAD)
- (file+olp PATH (OUTLINE ...))
- (file+head+olp PATH HEAD (OUTLINE ...))"
  :type 'sexp
  :group 'org-slipbox)

(defcustom org-slipbox-capture-ref-templates
  '(("r" "ref"
     :target (file+head
              "${slug}.org"
              "#+title: ${title}\n\n- source :: ${ref}\n\n${body}")
     :title "${title}"))
  "Capture templates used by ref-oriented capture workflows.
These templates use the same syntax as `org-slipbox-capture-templates'
and may interpolate `${ref}', `${body}', `${annotation}', and `${link}'."
  :type 'sexp
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-capture (&optional title)
  "Create a new note with TITLE and visit it."
  (interactive)
  (let* ((title (or title (read-string "Capture title: ")))
         (template (org-slipbox--read-capture-template org-slipbox-capture-templates)))
    (org-slipbox--visit-node (org-slipbox--capture-node title template))))

;;;###autoload
(defun org-slipbox-capture-ref (reference &optional title templates keys variables)
  "Visit the node for REFERENCE, or capture a new note with REFERENCE attached."
  (interactive (list (read-string "Ref: ")))
  (setq reference (string-trim reference))
  (when (string-empty-p reference)
    (user-error "Ref must not be empty"))
  (let* ((templates (or templates org-slipbox-capture-templates))
         (variables (plist-put (copy-sequence variables) :ref reference))
         (node (or (org-slipbox-node-from-ref reference)
                   (org-slipbox--capture-node
                    (or title (read-string "Capture title: "))
                    (org-slipbox--read-capture-template templates keys)
                    (list reference)
                    variables))))
    (org-slipbox--visit-node node)
    node))

;;;###autoload
(defun org-slipbox-capture-to-node (node heading)
  "Capture HEADING under existing NODE and visit the captured child."
  (interactive
   (list (org-slipbox--read-existing-node "Capture to node: ")
         (read-string "Heading: ")))
  (unless node
    (user-error "No target node selected"))
  (let ((captured (org-slipbox-rpc-append-heading-to-node
                   (plist-get node :node_key)
                   heading)))
    (org-slipbox--visit-node captured)
    captured))

(defun org-slipbox--select-or-capture-node (query)
  "Return a node selected for QUERY, or create one."
  (let* ((choices (org-slipbox--search-node-choices query))
         (create-choice (format "[Create] %s" query))
         (collection (append choices (list (cons create-choice :create))))
         (selection (completing-read "Node: " collection nil t nil nil create-choice))
         (choice (cdr (assoc selection collection))))
    (cond
     ((eq choice :create) (org-slipbox--capture-node query))
     (choice choice)
     (t nil))))

(defun org-slipbox--capture-node (title &optional template refs variables)
  "Capture a new node with TITLE using TEMPLATE, REFS, and VARIABLES."
  (org-slipbox--capture-node-at-time title template refs (current-time) variables))

(defun org-slipbox--capture-node-at-time (title &optional template refs time variables)
  "Capture a new node with TITLE using TEMPLATE, REFS, TIME, and VARIABLES."
  (let* ((template (or template (org-slipbox--default-capture-template)))
         (template-options (and template (nthcdr 2 template)))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             time
                             variables)
                            title))
         (target (org-slipbox--expand-capture-target
                  template-options
                  title
                  time
                  variables)))
    (pcase (plist-get target :kind)
      ('file
       (let ((params (if-let ((file-path (plist-get target :file_path)))
                         `(:title ,capture-title :file_path ,file-path)
                       `(:title ,capture-title))))
         (when-let ((head (plist-get target :head)))
           (setq params (plist-put params :head head)))
         (when refs
           (setq params (plist-put params :refs refs)))
         (org-slipbox-rpc-capture-node params)))
      ('file+olp
       (org-slipbox-rpc-append-heading-at-outline-path
        (append
         (list :file_path (plist-get target :file_path)
               :heading capture-title
               :outline_path (plist-get target :outline_path))
         (when-let ((head (plist-get target :head)))
           (list :head head)))))
      (_
       (user-error "Unsupported capture target")))))

(defun org-slipbox--default-capture-template ()
  "Return the default capture template."
  (car org-slipbox-capture-templates))

(defun org-slipbox--expand-capture-target (template-options title time &optional variables)
  "Expand TEMPLATE-OPTIONS for TITLE at TIME into a capture target plist."
  (let ((target (plist-get template-options :target)))
    (cond
     (target
      (pcase target
        (`(file ,path)
         `(:kind file
           :file_path ,(org-slipbox--expand-capture-template path title time variables)))
        (`(file+head ,path ,head)
         `(:kind file
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :head ,(org-slipbox--expand-capture-template head title time variables)))
        (`(file+olp ,path ,olp)
         `(:kind file+olp
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :outline_path ,(mapcar (lambda (heading)
                                    (org-slipbox--expand-capture-template
                                     heading title time variables))
                                  olp)))
        (`(file+head+olp ,path ,head ,olp)
         `(:kind file+olp
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :head ,(org-slipbox--expand-capture-template head title time variables)
           :outline_path ,(mapcar (lambda (heading)
                                    (org-slipbox--expand-capture-template
                                     heading title time variables))
                                  olp)))
        (_
         (user-error "Unsupported capture target %S" target))))
     ((plist-get template-options :path)
      `(:kind file
        :file_path ,(org-slipbox--expand-capture-template
                     (plist-get template-options :path)
                     title
                     time
                     variables)))
     (t
      '(:kind file)))))

(defun org-slipbox--read-capture-template (templates &optional keys)
  "Return a capture template from TEMPLATES, optionally selected by KEYS."
  (cond
   (keys
    (or (seq-find (lambda (template)
                    (equal (car template) keys))
                  templates)
        (user-error "No capture template with key %s" keys)))
   ((null templates) nil)
   ((= (length templates) 1)
    (car templates))
   (t
    (let* ((choices (mapcar (lambda (template)
                              (cons (format "%s %s" (car template) (cadr template))
                                    template))
                            templates))
           (selection (completing-read "Template: " choices nil t)))
      (cdr (assoc selection choices))))))

(defun org-slipbox--expand-capture-template (template title time &optional variables)
  "Expand TEMPLATE for TITLE using TIME and VARIABLES."
  (when template
    (let* ((context (org-slipbox--capture-template-context title variables))
           (expanded (replace-regexp-in-string
                      "%<\\([^>]+\\)>"
                      (lambda (match)
                        (format-time-string
                         (substring match 2 -1)
                         time))
                      template
                      t)))
      (replace-regexp-in-string
       "\\${\\([^}]+\\)}"
       (lambda (match)
         (if-let ((value (org-slipbox--capture-template-variable context match)))
             value
           match))
       expanded
       t
       t))))

(defun org-slipbox--capture-template-context (title variables)
  "Return template expansion variables for TITLE merged with VARIABLES."
  (let ((context (list :title title
                       :slug (org-slipbox--slugify title)
                       :ref ""
                       :body ""
                       :annotation ""
                       :link "")))
    (while variables
      (setq context (plist-put context (car variables) (cadr variables))
            variables (cddr variables)))
    context))

(defun org-slipbox--capture-template-variable (context match)
  "Return the replacement from CONTEXT for placeholder MATCH, or nil."
  (when (string-match "\\${\\([^}]+\\)}" match)
    (let ((key (intern (concat ":" (match-string 1 match)))))
      (when (plist-member context key)
        (format "%s" (or (plist-get context key) ""))))))

(defun org-slipbox--slugify (title)
  "Convert TITLE into a stable file-name slug."
  (let ((result "")
        (previous-dash nil))
    (dolist (character (string-to-list title))
      (let ((normalized (downcase character)))
        (cond
         ((or (and (<= ?a normalized) (<= normalized ?z))
              (and (<= ?0 normalized) (<= normalized ?9)))
          (setq result (concat result (string normalized))
                previous-dash nil))
         ((not previous-dash)
          (setq result (concat result "-")
                previous-dash t)))))
    (let ((trimmed (string-trim result "-+" "-+")))
      (if (string-empty-p trimmed)
          "note"
        trimmed))))

(provide 'org-slipbox-capture)

;;; org-slipbox-capture.el ends here
