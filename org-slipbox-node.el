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

(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-rpc)
(autoload 'org-slipbox--select-or-capture-node "org-slipbox-capture")

(defcustom org-slipbox-search-limit 50
  "Maximum number of nodes to request for interactive search."
  :type 'integer
  :group 'org-slipbox)

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

(defun org-slipbox-node-find (query)
  "Find a node matching QUERY and visit it."
  (interactive (list (read-string "Find node: ")))
  (let ((node (org-slipbox--select-or-capture-node query)))
    (when node
      (org-slipbox--visit-node node))))

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

(defun org-slipbox-node-insert (query)
  "Insert an `id:' link to a node selected using QUERY."
  (interactive (list (read-string "Insert node: ")))
  (let* ((node (org-slipbox--select-or-capture-node query))
         (node-with-id (and node (org-slipbox--ensure-node-id node)))
         (id (and node-with-id (plist-get node-with-id :explicit_id)))
         (title (and node-with-id (plist-get node-with-id :title))))
    (when node-with-id
      (insert (format "[[id:%s][%s]]" id title)))))

(defun org-slipbox-node-backlinks (query)
  "Show backlinks for a node selected using QUERY."
  (interactive (list (read-string "Backlinks for: ")))
  (let* ((node (org-slipbox--select-or-capture-node query))
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
                (insert (org-slipbox--node-display backlink) "\n"))
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
  (let* ((query (read-string prompt))
         (exact (and (not (string-empty-p query))
                     (condition-case nil
                         (org-slipbox-node-from-title-or-alias query t)
                       (error nil)))))
    (or exact
        (let* ((choices (org-slipbox--search-node-choices query))
               (selection (and choices
                               (completing-read "Node: " choices nil t))))
          (unless selection
            (user-error "No nodes match %s" query))
          (cdr (assoc selection choices))))))

(defun org-slipbox--search-node-choices (query)
  "Return display-to-node choices for QUERY."
  (let* ((response (org-slipbox-rpc-search-nodes query org-slipbox-search-limit))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes))))
    (mapcar (lambda (node)
              (cons (org-slipbox--node-display node) node))
            nodes)))

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
