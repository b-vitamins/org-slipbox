;;; org-slipbox-metadata.el --- Metadata commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Ref, alias, and tag metadata commands for `org-slipbox'.

;;; Code:

(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-tag-search-limit 200
  "Maximum number of indexed tags to request per completion query."
  :type 'integer
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-ref-find (query)
  "Find a node by reference QUERY and visit it."
  (interactive (list (read-string "Find ref: ")))
  (let* ((response (org-slipbox-rpc-search-refs query org-slipbox-search-limit))
         (refs (org-slipbox--plist-sequence (plist-get response :refs)))
         (choices (mapcar (lambda (entry)
                            (cons (org-slipbox--ref-display entry) entry))
                          refs)))
    (cond
     ((null refs)
      (user-error "No ref matches %s" query))
     ((= (length refs) 1)
      (org-slipbox--visit-node (plist-get (car refs) :node)))
     (t
      (let* ((selection (completing-read "Ref: " choices nil t))
             (entry (cdr (assoc selection choices))))
        (org-slipbox--visit-node (plist-get entry :node)))))))

;;;###autoload
(defun org-slipbox-ref-add (reference)
  "Add REFERENCE to the current node."
  (interactive (list (read-string "Ref: ")))
  (setq reference (string-trim reference))
  (when (string-empty-p reference)
    (user-error "ROAM_REFS must not be empty"))
  (let* ((node (org-slipbox--current-indexed-node))
         (current (org-slipbox--node-values node :refs))
         (updated (if (member reference current) current (append current (list reference)))))
    (org-slipbox--set-current-node-metadata node :refs updated)))

;;;###autoload
(defun org-slipbox-ref-remove (&optional reference)
  "Remove REFERENCE from the current node."
  (interactive)
  (let* ((node (org-slipbox--current-indexed-node))
         (references (org-slipbox--node-values node :refs))
         (reference (or reference
                        (and references
                             (completing-read "Ref: " references nil t)))))
    (unless reference
      (user-error "No ref to remove"))
    (org-slipbox--set-current-node-metadata
     node
     :refs
     (delete reference (copy-sequence references)))))

;;;###autoload
(defun org-slipbox-alias-add (alias)
  "Add ALIAS to the current node."
  (interactive (list (read-string "Alias: ")))
  (setq alias (string-trim alias))
  (when (string-empty-p alias)
    (user-error "ROAM_ALIASES must not be empty"))
  (let* ((node (org-slipbox--current-indexed-node))
         (current (org-slipbox--node-values node :aliases))
         (updated (if (member alias current) current (append current (list alias)))))
    (org-slipbox--set-current-node-metadata node :aliases updated)))

;;;###autoload
(defun org-slipbox-alias-remove (&optional alias)
  "Remove ALIAS from the current node."
  (interactive)
  (let* ((node (org-slipbox--current-indexed-node))
         (aliases (org-slipbox--node-values node :aliases))
         (alias (or alias
                    (and aliases
                         (completing-read "Alias: " aliases nil t)))))
    (unless alias
      (user-error "No alias to remove"))
    (org-slipbox--set-current-node-metadata
     node
     :aliases
     (delete alias (copy-sequence aliases)))))

(defun org-slipbox-tag-completions (&optional prefix)
  "Return known tags for completion.
When PREFIX is non-nil, only return tags matching PREFIX."
  (delete-dups
   (append (org-slipbox--indexed-tags (or prefix "") 10000)
           (org-slipbox--matching-org-tags prefix))))

;;;###autoload
(defun org-slipbox-tag-add (tags)
  "Add TAGS to the current node."
  (interactive
   (list
    (let ((crm-separator "[ \t]*:[ \t]*"))
      (completing-read-multiple "Tag: " #'org-slipbox--tag-completion-table nil nil))))
  (setq tags (delete-dups (seq-filter #'identity (mapcar #'string-trim tags))))
  (unless tags
    (user-error "No tag to add"))
  (let* ((node (org-slipbox--current-indexed-node))
         (current (org-slipbox--node-values node :tags))
         (updated (delete-dups (append tags current))))
    (org-slipbox--set-current-node-metadata node :tags updated)))

;;;###autoload
(defun org-slipbox-tag-remove (&optional tags)
  "Remove TAGS from the current node."
  (interactive)
  (let* ((node (org-slipbox--current-indexed-node))
         (current-tags (org-slipbox--node-values node :tags))
         (tags (or tags
                   (and current-tags
                        (completing-read-multiple "Tag: " current-tags nil t)))))
    (unless current-tags
      (user-error "No tag to remove"))
    (setq tags (delete-dups (seq-filter #'identity (mapcar #'string-trim tags))))
    (unless tags
      (user-error "No tag selected"))
    (org-slipbox--set-current-node-metadata
     node
     :tags
     (seq-remove (lambda (tag) (member tag tags)) current-tags))))

(defun org-slipbox--current-indexed-node ()
  "Return the indexed node at point, syncing the buffer if needed."
  (org-slipbox-node-at-point t))

(defun org-slipbox--set-current-node-metadata (node field values)
  "Set FIELD on NODE to VALUES through the Rust RPC layer."
  (let ((path (org-slipbox--current-node-buffer-file)))
    (org-slipbox-rpc-update-node-metadata
     (append
      (list :node_key (plist-get node :node_key))
      (pcase field
        (:aliases (list :aliases values))
        (:refs (list :refs values))
        (:tags (list :tags values))
        (_ (error "Unsupported metadata field: %s" field)))))
    (org-slipbox--refresh-live-file-buffer path)
    values))

(defun org-slipbox--node-values (node key)
  "Return KEY values from NODE as a list."
  (org-slipbox--plist-sequence (plist-get node key)))

(defun org-slipbox--ref-display (entry)
  "Return a display string for reference ENTRY."
  (format "%s | %s"
          (plist-get entry :reference)
          (org-slipbox--node-display (plist-get entry :node))))

(defun org-slipbox--tag-completion-table (string pred action)
  "Completion table for tags using STRING, PRED, and ACTION."
  (complete-with-action
   action
   (delete-dups
    (append (org-slipbox--indexed-tags string org-slipbox-tag-search-limit)
            (org-slipbox--matching-org-tags string)))
   string
   pred))

(defun org-slipbox--indexed-tags (query limit)
  "Return indexed tags matching QUERY, requesting up to LIMIT results."
  (let* ((response (org-slipbox-rpc-search-tags query limit))
         (tags (plist-get response :tags)))
    (org-slipbox--plist-sequence tags)))

(defun org-slipbox--matching-org-tags (&optional prefix)
  "Return `org-tag-alist' tags matching PREFIX."
  (let ((prefix (or prefix "")))
    (seq-filter
     (lambda (tag)
       (or (string-empty-p prefix)
           (string-prefix-p prefix tag completion-ignore-case)))
     (delete-dups
      (delq nil
            (mapcar (lambda (entry)
                      (pcase entry
                        (`(,tag . ,_) (and (stringp tag) tag))
                        (_ nil)))
                    org-tag-alist))))))

(provide 'org-slipbox-metadata)

;;; org-slipbox-metadata.el ends here
