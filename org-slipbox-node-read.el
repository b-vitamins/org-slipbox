;;; org-slipbox-node-read.el --- Node completion for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.5.0
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

;; Completion, formatting, and selection helpers for `org-slipbox' nodes.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-rpc)

(defvar org-slipbox-directory)

(defcustom org-slipbox-search-limit 50
  "Maximum number of nodes to request for interactive search."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-node-read-limit 200
  "Maximum number of indexed nodes to request for interactive completion."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-node-display-template #'org-slipbox--node-display
  "Formatting used for interactive node completion candidates.

When this is a function, it is called with one NODE plist and must
return the display string.

When this is a string, patterns of the form `${field}' or
`${field:length}' are expanded from the current node. Supported fields
include `title', `outline', `olp', `tags', `aliases', `refs', `todo', `kind',
`file', `line', `id', and indexed metadata fields such as `modtime',
`file-mtime', `mtime-ns', `backlinks', and `forward-links'. Migration-friendly
aliases such as `backlinkscount' and `forwardlinkscount' are also accepted. A
`length' of `*' uses the remaining candidate width; an integer width pads or
truncates the rendered field."
  :type '(choice function string)
  :group 'org-slipbox)

(defcustom org-slipbox-node-default-sort nil
  "Default sort order for `org-slipbox-node-read'.
When nil, preserve the Rust query engine ranking."
  :type '(choice
          (const :tag "Engine ranking" nil)
          (const :tag "Title" title)
          (const :tag "File path" file)
          (const :tag "File mtime" file-mtime)
          (const :tag "Backlink count" backlink-count)
          (const :tag "Forward-link count" forward-link-count)
          (function :tag "Custom comparator"))
  :group 'org-slipbox)

(defcustom org-slipbox-node-annotation-function #'org-slipbox-node-read--annotation
  "Function used to annotate `org-slipbox-node-read' completion candidates.
The function receives one NODE plist and must return a string.
Indexed node payloads include daemon-owned metadata such as
`:file_mtime_ns', `:backlink_count', and `:forward_link_count'."
  :type 'function
  :group 'org-slipbox)

(defcustom org-slipbox-node-formatter nil
  "Formatting used when inserting link descriptions for nodes.
When nil, inserted descriptions default to the node title. When this is
a function, it is called with one NODE plist. When it is a string, it
uses the same placeholder syntax as `org-slipbox-node-display-template'."
  :type '(choice
          (const :tag "Node title" nil)
          function
          string)
  :group 'org-slipbox)

(defcustom org-slipbox-node-template-prefixes
  '(("tags" . "#")
    ("todo" . "t:"))
  "Prefixes used for string-template node fields."
  :type '(alist :key-type string :value-type string)
  :group 'org-slipbox)

(defvar org-slipbox-node-history nil
  "Minibuffer history for `org-slipbox-node-read'.")

(defun org-slipbox-node-read
    (&optional initial-input filter-fn sort-fn require-match prompt)
  "Read and return an indexed node plist.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes.
SORT-FN names an engine-backed sort or provides a custom comparator.
REQUIRE-MATCH enforces an indexed selection. PROMPT defaults to
\"Node: \". When REQUIRE-MATCH is nil and the user enters a new title,
return a plist with only `:title'."
  (let* ((prompt (or prompt "Node: "))
         (sort (org-slipbox--resolve-node-sort sort-fn))
         completions
         (collection
          (lambda (string pred action)
            (if (eq action 'metadata)
                `(metadata
                  ,@(when sort
                      '((display-sort-function . identity)
                        (cycle-sort-function . identity)))
                  (annotation-function
                   . ,(lambda (title)
                        (org-slipbox-node-completion-annotation title)))
                  (category . org-slipbox-node))
              (setq completions
                    (org-slipbox-node-completion-candidates string filter-fn sort))
              (complete-with-action action completions string pred))))
         (selection (completing-read
                     prompt
                     collection
                     nil
                     require-match
                     initial-input
                     'org-slipbox-node-history))
         (node (cdr (assoc selection completions))))
    (cond
     (node node)
     ((string-empty-p selection) nil)
     (t
      (let ((refreshed (cdr (assoc selection
                                   (org-slipbox-node-completion-candidates
                                    selection
                                    filter-fn
                                    sort)))))
        (or refreshed
            (and (not require-match)
                 (list :title selection))))))))

(defun org-slipbox-node-formatted (node)
  "Return the preferred inserted-link description for NODE."
  (let ((formatted
         (cond
          ((functionp org-slipbox-node-formatter)
           (funcall org-slipbox-node-formatter node))
          ((stringp org-slipbox-node-formatter)
           (org-slipbox--format-node-template
            org-slipbox-node-formatter
            node
            (frame-width)))
          (t
           (plist-get node :title)))))
    (if (string-empty-p (or formatted ""))
        (plist-get node :title)
      formatted)))

(defun org-slipbox--read-existing-node (prompt)
  "Read and return an existing node using PROMPT."
  (or (org-slipbox-node-read nil nil nil t prompt)
      (user-error "No node selected")))

(defun org-slipbox-node-completion-candidates (query &optional filter-fn sort-fn)
  "Return formatted node completion candidates for QUERY.
FILTER-FN filters indexed nodes. SORT-FN names an engine-backed sort or
provides a custom comparator."
  (let* ((sort (org-slipbox--resolve-node-sort sort-fn))
         (response (org-slipbox-rpc-search-nodes
                    query
                    org-slipbox-node-read-limit
                    (org-slipbox--node-sort-rpc-value sort)))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
         (nodes (if filter-fn
                    (seq-filter filter-fn nodes)
                  nodes))
         (completions (mapcar #'org-slipbox--node-completion-candidate nodes)))
    (if-let ((comparator (org-slipbox--node-sort-local-comparator sort)))
        (seq-sort comparator completions)
      completions)))

(defun org-slipbox-node-read--completions (query &optional filter-fn sort)
  "Return formatted completion candidates for QUERY.
FILTER-FN filters indexed nodes. SORT configures ordering."
  (org-slipbox-node-completion-candidates query filter-fn sort))

(defun org-slipbox-node-completion-annotation (candidate)
  "Return the annotation string for node completion CANDIDATE."
  (if-let ((node (get-text-property 0 'node candidate)))
      (funcall org-slipbox-node-annotation-function node)
    ""))

(defun org-slipbox--search-node-choices (query)
  "Return display-to-node choices for QUERY."
  (org-slipbox-node-completion-candidates query))

(defun org-slipbox--node-completion-candidate (node)
  "Return a display-to-node completion pair for NODE."
  (cons (propertize (org-slipbox--node-candidate-display node) 'node node)
        node))

(defun org-slipbox--node-candidate-display (node)
  "Return the completion-candidate display string for NODE."
  (cond
   ((functionp org-slipbox-node-display-template)
    (funcall org-slipbox-node-display-template node))
   ((stringp org-slipbox-node-display-template)
    (org-slipbox--format-node-template
     org-slipbox-node-display-template
     node
     (frame-width)))
   (t
    (org-slipbox--node-display node))))

(defalias 'org-slipbox--node-completion-annotation
  #'org-slipbox-node-completion-annotation)

(defun org-slipbox--node-display (node)
  "Return a display string for NODE."
  (let ((title (plist-get node :title))
        (outline (plist-get node :outline_path))
        (tags (org-slipbox--plist-sequence (plist-get node :tags)))
        (file (plist-get node :file_path))
        (line (plist-get node :line)))
    (string-join
     (delq nil
           (list title
                 (unless (string-empty-p outline) outline)
                 (unless (null tags)
                   (string-join (mapcar (lambda (tag) (format "#%s" tag)) tags) " "))
                 (format "%s:%s" file line)))
     " | ")))

(defun org-slipbox--format-node-template (template node width)
  "Format TEMPLATE for NODE within WIDTH columns."
  (let ((star-width (org-slipbox--node-template-star-width template node width)))
    (replace-regexp-in-string
     "\\${\\([^}:]+\\)\\(?::\\([^}]+\\)\\)?}"
     (lambda (match)
       (pcase-let* ((`(,field ,length)
                     (org-slipbox--node-template-placeholder match))
                    (value (org-slipbox--node-template-value node field))
                    (target-width (cond
                                   ((null length) nil)
                                   ((string-equal length "*") star-width)
                                   (t (string-to-number length)))))
         (if target-width
             (truncate-string-to-width value target-width 0 ?\s)
           value)))
     template
     t
     t)))

(defun org-slipbox--node-template-star-width (template node width)
  "Return the width to use for `*' placeholders in TEMPLATE for NODE."
  (max 0 (- width (org-slipbox--node-template-min-width template node))))

(defun org-slipbox--node-template-min-width (template node)
  "Return the fixed-width portion of TEMPLATE for NODE."
  (let ((cursor 0)
        (total 0))
    (while (string-match "\\${\\([^}:]+\\)\\(?::\\([^}]+\\)\\)?}" template cursor)
      (setq total (+ total
                     (string-width (substring template cursor (match-beginning 0)))))
      (pcase-let* ((field (match-string 1 template))
                   (length (match-string 2 template))
                   (value (org-slipbox--node-template-value node field)))
        (setq total (+ total
                       (cond
                        ((null length) (string-width value))
                        ((string-equal length "*") 0)
                        (t (string-to-number length))))))
      (setq cursor (match-end 0)))
    (+ total (string-width (substring template cursor)))))

(defun org-slipbox--node-template-placeholder (match)
  "Return the field and width specifier parsed from MATCH."
  (when (string-match "\\${\\([^}:]+\\)\\(?::\\([^}]+\\)\\)?}" match)
    (list (match-string 1 match)
          (match-string 2 match))))

(defun org-slipbox--node-template-value (node field)
  "Return NODE rendered for template FIELD."
  (pcase field
    ("title" (or (plist-get node :title) ""))
    ((or "outline" "olp") (or (plist-get node :outline_path) ""))
    ("tags" (org-slipbox--node-template-list-value
             "tags"
             (org-slipbox--plist-sequence (plist-get node :tags))))
    ("aliases" (string-join (org-slipbox--plist-sequence (plist-get node :aliases)) ", "))
    ("refs" (string-join (org-slipbox--plist-sequence (plist-get node :refs)) ", "))
    ("todo" (org-slipbox--node-template-list-value
             "todo"
             (org-slipbox--plist-sequence (plist-get node :todo_keyword))))
    ("kind" (pcase (plist-get node :kind)
              ('file "file")
              ('heading "heading")
              ((pred stringp) (plist-get node :kind))
              (_ "")))
    ("file" (or (plist-get node :file_path) ""))
    ("line" (if-let ((line (plist-get node :line)))
                (number-to-string line)
              ""))
    ("id" (or (plist-get node :explicit_id) ""))
    ((or "modtime" "mtime" "file-mtime")
     (org-slipbox--node-template-file-mtime-value node))
    ((or "mtime-ns" "file-mtime-ns")
     (org-slipbox--node-template-number-value (plist-get node :file_mtime_ns)))
    ((or "backlinks" "backlink-count" "backlinkscount")
     (org-slipbox--node-template-number-value (plist-get node :backlink_count)))
    ((or "forward-links" "forward-link-count" "forwardlinkscount")
     (org-slipbox--node-template-number-value
      (plist-get node :forward_link_count)))
    (_ "")))

(defun org-slipbox--node-template-number-value (value)
  "Return VALUE rendered as a decimal string when numeric."
  (if (numberp value)
      (number-to-string value)
    ""))

(defun org-slipbox--node-template-file-mtime-value (node)
  "Return NODE file modification time rendered for display templates."
  (let ((mtime-ns (plist-get node :file_mtime_ns)))
    (if (and (integerp mtime-ns)
             (> mtime-ns 0))
        (format-time-string
         "%Y-%m-%d"
         (seconds-to-time (/ (float mtime-ns) 1000000000.0)))
      "")))

(defun org-slipbox--node-template-list-value (field values)
  "Return VALUES joined for template FIELD."
  (let ((prefix (or (cdr (assoc field org-slipbox-node-template-prefixes)) "")))
    (string-join
     (mapcar (lambda (value)
               (concat prefix value))
             values)
     " ")))

(defun org-slipbox--resolve-node-sort (sort-fn)
  "Return the effective sort configuration for SORT-FN."
  (let ((sort-fn (or sort-fn org-slipbox-node-default-sort)))
    (cond
     ((functionp sort-fn) sort-fn)
     ((null sort-fn) nil)
     ((memq sort-fn '(relevance title file file-mtime backlink-count forward-link-count))
      sort-fn)
     (t
      (user-error "Unsupported node sort %s" sort-fn)))))

(defun org-slipbox--node-sort-rpc-value (sort)
  "Return the daemon sort name for SORT, or nil when local sorting applies."
  (when (memq sort '(relevance title file file-mtime backlink-count forward-link-count))
    sort))

(defun org-slipbox--node-sort-local-comparator (sort)
  "Return the local comparator for SORT, or nil when the daemon sorts."
  (when (functionp sort)
    sort))

(defun org-slipbox-node-read-sort-by-title (completion-a completion-b)
  "Sort COMPLETION-A and COMPLETION-B by title."
  (string-collate-lessp
   (plist-get (cdr completion-a) :title)
   (plist-get (cdr completion-b) :title)
   nil
   t))

(defun org-slipbox-node-read-sort-by-file (completion-a completion-b)
  "Sort COMPLETION-A and COMPLETION-B by file path, then line number."
  (let ((file-a (plist-get (cdr completion-a) :file_path))
        (file-b (plist-get (cdr completion-b) :file_path)))
    (if (string-equal file-a file-b)
        (< (plist-get (cdr completion-a) :line)
           (plist-get (cdr completion-b) :line))
      (string-collate-lessp file-a file-b nil t))))

(defun org-slipbox-node-read-sort-by-file-mtime (completion-a completion-b)
  "Sort COMPLETION-A and COMPLETION-B by descending indexed file mtime."
  (let ((mtime-a (or (plist-get (cdr completion-a) :file_mtime_ns) 0))
        (mtime-b (or (plist-get (cdr completion-b) :file_mtime_ns) 0)))
    (if (/= mtime-a mtime-b)
        (> mtime-a mtime-b)
      (org-slipbox-node-read-sort-by-file completion-a completion-b))))

(defun org-slipbox-node-read--annotation (_node)
  "Default empty annotation for node completions."
  "")

(defun org-slipbox--title-completion-table (string pred action)
  "Completion table for node titles and aliases using STRING, PRED, and ACTION."
  (if (eq action 'metadata)
      '(metadata (category . org-slipbox-node))
    (complete-with-action
     action
     (org-slipbox--title-completion-candidates string)
     string
     pred)))

(defun org-slipbox--title-completion-candidates (query)
  "Return title and alias candidates matching QUERY."
  (let* ((response (org-slipbox-rpc-search-nodes query org-slipbox-search-limit))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
         candidates)
    (dolist (node nodes)
      (dolist (candidate (delete-dups
                          (append (list (plist-get node :title))
                                  (org-slipbox--plist-sequence (plist-get node :aliases)))))
        (when (and (stringp candidate)
                   (or (string-empty-p query)
                       (string-prefix-p query candidate completion-ignore-case)))
          (push candidate candidates))))
    (delete-dups (nreverse candidates))))

(provide 'org-slipbox-node-read)

;;; org-slipbox-node-read.el ends here
