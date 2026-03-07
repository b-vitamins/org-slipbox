;;; org-slipbox-node.el --- Node lookup commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Node lookup and display helpers for `org-slipbox'.

;;; Code:

(require 'cl-lib)
(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-rpc)
(autoload 'org-slipbox--capture-node "org-slipbox-capture")

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
`file', `line', and `id'. A `length' of `*' uses the remaining candidate width; an
integer width pads or truncates the rendered field."
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
          (const :tag "File atime" file-atime)
          (function :tag "Custom comparator"))
  :group 'org-slipbox)

(defcustom org-slipbox-node-annotation-function #'org-slipbox-node-read--annotation
  "Function used to annotate `org-slipbox-node-read' completion candidates.
The function receives one NODE plist and must return a string."
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

(defun org-slipbox-index ()
  "Rebuild the local org-slipbox index from Org files."
  (interactive)
  (let* ((response (org-slipbox-rpc-index))
         (files (plist-get response :files_indexed))
         (nodes (plist-get response :nodes_indexed))
         (links (plist-get response :links_indexed)))
    (message "Indexed %s files, %s nodes, %s links" files nodes links)
    response))

(defun org-slipbox-node-from-id (id)
  "Return the indexed node with explicit ID."
  (org-slipbox-rpc-node-from-id id))

(defun org-slipbox-node-from-title-or-alias (title-or-alias &optional nocase)
  "Return the indexed node matching TITLE-OR-ALIAS.
When NOCASE is non-nil, use case-insensitive matching."
  (org-slipbox-rpc-node-from-title-or-alias title-or-alias nocase))

(defun org-slipbox-node-from-ref (reference)
  "Return the indexed node for REFERENCE, or nil when none exists."
  (org-slipbox-rpc-node-from-ref reference))

(defun org-slipbox-node-at-point (&optional assert)
  "Return the indexed node at point.
If ASSERT is non-nil, signal a user error when no node is available."
  (let ((node (and (org-slipbox--queryable-node-buffer-p)
                   (progn
                     (org-slipbox--sync-node-buffer-if-needed)
                     (org-slipbox-rpc-node-at-point
                      (org-slipbox--current-node-buffer-file)
                      (line-number-at-pos))))))
    (or node
        (and assert (user-error "No node at point")))))

(defun org-slipbox-node-find (&optional initial-input filter-fn other-window)
  "Find a node, creating it if needed.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes.
With OTHER-WINDOW, visit the result in another window."
  (interactive (list nil nil current-prefix-arg))
  (let ((node (org-slipbox-node-read initial-input filter-fn nil nil "Node: ")))
    (when node
      (if (plist-get node :file_path)
          (org-slipbox--visit-node node other-window)
        (org-slipbox--visit-node
         (org-slipbox--capture-node (plist-get node :title))
         other-window)))))

;;;###autoload
(defun org-slipbox-node-random (&optional other-window)
  "Visit a random indexed node.
With prefix argument OTHER-WINDOW, visit it in another window."
  (interactive "P")
  (let* ((response (org-slipbox-rpc-random-node))
         (node (plist-get response :node)))
    (unless node
      (user-error "No indexed nodes available"))
    (org-slipbox--visit-node node other-window)
    node))

(defun org-slipbox-node-insert (&optional initial-input filter-fn)
  "Insert an `id:' link to a selected node.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes."
  (interactive)
  (let* ((node (org-slipbox-node-read initial-input filter-fn nil nil "Node: "))
         (node (and node
                    (if (plist-get node :file_path)
                        node
                      (org-slipbox--capture-node (plist-get node :title)))))
         (node-with-id (and node (org-slipbox--ensure-node-id node)))
         (id (and node-with-id (plist-get node-with-id :explicit_id)))
         (title (and node-with-id (org-slipbox-node-formatted node-with-id))))
    (when node-with-id
      (insert (format "[[id:%s][%s]]" id title)))))

(defun org-slipbox-node-backlinks (&optional initial-input filter-fn)
  "Show backlinks for a selected existing node.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes."
  (interactive)
  (let* ((node (org-slipbox-node-read
                initial-input
                filter-fn
                nil
                t
                "Backlinks for node: "))
         (node-key (plist-get node :node_key))
         (response (and node (org-slipbox-rpc-backlinks node-key 200)))
         (backlinks (and response
                         (org-slipbox--plist-sequence
                          (plist-get response :backlinks)))))
    (when node
      (with-current-buffer (get-buffer-create "*org-slipbox backlinks*")
        (let ((inhibit-read-only t))
          (erase-buffer)
          (special-mode)
          (insert (format "Backlinks for %s\n\n" (plist-get node :title)))
          (if backlinks
              (dolist (backlink backlinks)
                (insert
                 (org-slipbox--node-display (plist-get backlink :source_node))
                 "\n  "
                 (format "%s:%s:%s"
                         (plist-get (plist-get backlink :source_node) :file_path)
                         (plist-get backlink :row)
                         (plist-get backlink :col))
                 " "
                 (plist-get backlink :preview)
                 "\n"))
            (insert "No backlinks found.\n")))
        (display-buffer (current-buffer))))))

(defun org-slipbox--ensure-node-id (node)
  "Return NODE with an explicit ID, assigning one if needed."
  (if (plist-get node :explicit_id)
      node
    (org-slipbox-rpc-ensure-node-id (plist-get node :node_key))))

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

(defun org-slipbox-node-read (&optional initial-input filter-fn sort-fn require-match prompt)
  "Read and return an indexed node plist.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes.
SORT-FN sorts completion candidates. REQUIRE-MATCH enforces an indexed
selection. PROMPT defaults to \"Node: \". When REQUIRE-MATCH is nil and
the user enters a new title, return a plist with only `:title'."
  (let* ((prompt (or prompt "Node: "))
         (sort-fn (org-slipbox--resolve-node-sort-function sort-fn))
         completions
         (collection
          (lambda (string pred action)
            (if (eq action 'metadata)
                `(metadata
                  ,@(when sort-fn
                      '((display-sort-function . identity)
                        (cycle-sort-function . identity)))
                  (annotation-function
                   . ,(lambda (title)
                        (org-slipbox--node-completion-annotation title)))
                  (category . org-slipbox-node))
              (setq completions
                    (org-slipbox-node-read--completions string filter-fn sort-fn))
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
                                   (org-slipbox-node-read--completions
                                    selection
                                    filter-fn
                                    sort-fn)))))
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

(defun org-slipbox--plist-sequence (value)
  "Normalize JSON-derived VALUE into an Emacs list."
  (cond
   ((null value) nil)
   ((vectorp value) (append value nil))
   ((listp value) value)
   (t (list value))))

(defun org-slipbox--save-and-sync-current-buffer ()
  "Save and sync the current buffer into the index."
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (org-slipbox--save-and-sync-buffer (current-buffer)))

(defun org-slipbox--save-and-sync-buffer (buffer)
  "Save BUFFER and sync it into the index."
  (with-current-buffer buffer
    (unless buffer-file-name
      (user-error "Current buffer is not visiting a file"))
    (save-buffer)
    (org-slipbox--sync-file-path buffer-file-name)))

(defun org-slipbox--sync-file-path (path)
  "Sync absolute PATH into the index."
  (org-slipbox-rpc-index-file path))

(defun org-slipbox--normalize-file-path (path)
  "Return an absolute normalized form of PATH."
  (and path (expand-file-name path)))

(defun org-slipbox--live-file-buffer (path)
  "Return a live buffer visiting PATH, or nil."
  (let ((absolute (org-slipbox--normalize-file-path path)))
    (seq-find
     (lambda (buffer)
       (let* ((base-buffer (or (buffer-base-buffer buffer) buffer))
              (buffer-path (buffer-file-name base-buffer)))
         (and buffer-path
              (equal (org-slipbox--normalize-file-path buffer-path) absolute))))
     (buffer-list))))

(defun org-slipbox--sync-live-file-buffer-if-needed (path)
  "Save and sync the live buffer visiting PATH when needed."
  (let ((buffer (org-slipbox--live-file-buffer path)))
    (when buffer
      (with-current-buffer (or (buffer-base-buffer buffer) buffer)
        (when (buffer-modified-p)
          (org-slipbox--save-and-sync-current-buffer))))))

(defun org-slipbox--refresh-live-file-buffer (path)
  "Revert the live buffer visiting PATH, if any."
  (let ((buffer (org-slipbox--live-file-buffer path)))
    (when buffer
      (with-current-buffer (or (buffer-base-buffer buffer) buffer)
        (revert-buffer :ignore-auto :noconfirm)))))

(defun org-slipbox--kill-live-file-buffer (path)
  "Kill the live buffer visiting PATH, if any."
  (let ((buffer (org-slipbox--live-file-buffer path)))
    (when buffer
      (kill-buffer (or (buffer-base-buffer buffer) buffer)))))

(defun org-slipbox--sync-node-buffer-if-needed ()
  "Save and sync the current node buffer when it has local modifications."
  (let ((base-buffer (or (buffer-base-buffer) (current-buffer))))
    (when (and (org-slipbox--queryable-node-buffer-p)
               (buffer-modified-p base-buffer))
      (with-current-buffer base-buffer
        (org-slipbox--save-and-sync-current-buffer)))))

(defun org-slipbox--queryable-node-buffer-p ()
  "Return non-nil when the current buffer can resolve indexed nodes."
  (let ((file (org-slipbox--current-node-buffer-file)))
    (and file
         org-slipbox-directory
         (string-suffix-p ".org" file)
         (file-in-directory-p (expand-file-name file)
                              (expand-file-name org-slipbox-directory)))))

(defun org-slipbox--current-node-buffer-file ()
  "Return the current base buffer file path."
  (buffer-file-name (or (buffer-base-buffer) (current-buffer))))

(defun org-slipbox--read-existing-node (prompt)
  "Read and return an existing node using PROMPT."
  (or (org-slipbox-node-read nil nil nil t prompt)
      (user-error "No node selected")))

(defun org-slipbox-node-read--completions (query &optional filter-fn sort-fn)
  "Return formatted completion candidates for QUERY.
FILTER-FN filters indexed nodes. SORT-FN sorts completion candidates."
  (let* ((response (org-slipbox-rpc-search-nodes query org-slipbox-node-read-limit))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
         (nodes (if filter-fn
                    (seq-filter filter-fn nodes)
                  nodes))
         (completions (mapcar #'org-slipbox--node-completion-candidate nodes)))
    (if sort-fn
        (seq-sort sort-fn completions)
      completions)))

(defun org-slipbox--search-node-choices (query)
  "Return display-to-node choices for QUERY."
  (org-slipbox-node-read--completions query))

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

(defun org-slipbox--node-completion-annotation (candidate)
  "Return the annotation string for completion CANDIDATE."
  (if-let ((node (get-text-property 0 'node candidate)))
      (funcall org-slipbox-node-annotation-function node)
    ""))

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
    (_ "")))

(defun org-slipbox--node-template-list-value (field values)
  "Return VALUES joined for template FIELD."
  (let ((prefix (or (cdr (assoc field org-slipbox-node-template-prefixes)) "")))
    (string-join
     (mapcar (lambda (value)
               (concat prefix value))
             values)
     " ")))

(defun org-slipbox--resolve-node-sort-function (sort-fn)
  "Return the effective sort function for SORT-FN."
  (let ((sort-fn (or sort-fn org-slipbox-node-default-sort)))
    (cond
     ((functionp sort-fn) sort-fn)
     ((null sort-fn) nil)
     ((fboundp (intern-soft (format "org-slipbox-node-read-sort-by-%s" sort-fn)))
      (intern-soft (format "org-slipbox-node-read-sort-by-%s" sort-fn)))
     (t
      (user-error "Unsupported node sort %s" sort-fn)))))

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
  "Sort COMPLETION-A and COMPLETION-B by descending file modification time."
  (time-less-p (org-slipbox--node-file-time (cdr completion-b) 'modification)
               (org-slipbox--node-file-time (cdr completion-a) 'modification)))

(defun org-slipbox-node-read-sort-by-file-atime (completion-a completion-b)
  "Sort COMPLETION-A and COMPLETION-B by descending file access time."
  (time-less-p (org-slipbox--node-file-time (cdr completion-b) 'access)
               (org-slipbox--node-file-time (cdr completion-a) 'access)))

(defun org-slipbox--node-file-time (node attribute)
  "Return NODE file ATTRIBUTE time, or the epoch when unavailable."
  (let* ((file (expand-file-name (plist-get node :file_path) org-slipbox-directory))
         (attributes (and (file-exists-p file)
                          (file-attributes file 'string))))
    (or (pcase attribute
          ('modification (file-attribute-modification-time attributes))
          ('access (file-attribute-access-time attributes)))
        '(0 0 0 0))))

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

(defun org-slipbox--visit-node (node &optional other-window)
  "Visit NODE in its source file.
With OTHER-WINDOW, visit it in another window."
  (funcall (if other-window #'find-file-other-window #'find-file)
           (expand-file-name (plist-get node :file_path) org-slipbox-directory))
  (goto-char (point-min))
  (forward-line (1- (plist-get node :line))))

(provide 'org-slipbox-node)

;;; org-slipbox-node.el ends here
