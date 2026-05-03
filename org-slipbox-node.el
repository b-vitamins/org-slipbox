;;; org-slipbox-node.el --- Node lookup commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.7.0
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

;; Public node command facade for `org-slipbox'.

;;; Code:

(require 'org-slipbox-node-insert)
(require 'org-slipbox-node-read)
(require 'org-slipbox-node-visit)
(require 'org-slipbox-rpc)
(autoload 'org-slipbox--capture-node "org-slipbox-capture")

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
  "Return the canonical node with explicit ID."
  (org-slipbox-rpc-node-from-id id))

(defun org-slipbox-node-from-title-or-alias (title-or-alias &optional nocase)
  "Return the canonical node matching TITLE-OR-ALIAS.
When NOCASE is non-nil, use case-insensitive matching."
  (org-slipbox-rpc-node-from-title-or-alias title-or-alias nocase))

(defun org-slipbox-node-from-ref (reference)
  "Return the canonical node for REFERENCE, or nil when none exists."
  (org-slipbox-rpc-node-from-ref reference))

(defun org-slipbox-node-find (&optional initial-input filter-fn other-window)
  "Find a node, creating it if needed.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes.
With OTHER-WINDOW, visit the result in another window."
  (interactive (list nil nil current-prefix-arg))
  (let* ((node (org-slipbox-node-read initial-input filter-fn nil nil "Node: "))
         (finalize (if other-window
                       (lambda (captured _session)
                         (org-slipbox--visit-node captured t))
                     'find-file)))
    (when node
      (if (plist-get node :file_path)
          (org-slipbox--visit-node node other-window)
        (org-slipbox--capture-node
         (plist-get node :title)
         nil
         nil
         nil
         `(:default-finalize ,finalize))))))

;;;###autoload
(defun org-slipbox-node-random (&optional other-window)
  "Visit a random canonical node.
With prefix argument OTHER-WINDOW, visit it in another window."
  (interactive "P")
  (let* ((response (org-slipbox-rpc-random-node))
         (node (plist-get response :node)))
    (unless node
      (user-error "No canonical nodes available"))
    (org-slipbox--visit-node node other-window)
    node))

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
      (org-slipbox-node--display-link-occurrences
       "*org-slipbox backlinks*"
       (format "Backlinks for %s" (plist-get node :title))
       backlinks
       :source_note
       "No backlinks found."))))

(defun org-slipbox-node-forward-links (&optional initial-input filter-fn)
  "Show forward links for a selected existing node.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes."
  (interactive)
  (let* ((node (org-slipbox-node-read
                initial-input
                filter-fn
                nil
                t
                "Forward links for node: "))
         (node-key (plist-get node :node_key))
         (response (and node (org-slipbox-rpc-forward-links node-key 200)))
         (forward-links (and response
                             (org-slipbox--plist-sequence
                              (plist-get response :forward_links)))))
    (when node
      (org-slipbox-node--display-link-occurrences
       "*org-slipbox forward-links*"
       (format "Forward links for %s" (plist-get node :title))
       forward-links
       :destination_note
       "No forward links found."))))

(defun org-slipbox-node--display-link-occurrences
    (buffer-name heading records related-node-slot empty-message)
  "Render link RECORDS into BUFFER-NAME under HEADING.
RELATED-NODE-SLOT names the nested node plist in each record.
EMPTY-MESSAGE is shown when RECORDS is empty."
  (with-current-buffer (get-buffer-create buffer-name)
    (let ((inhibit-read-only t))
      (erase-buffer)
      (special-mode)
      (insert heading "\n\n")
      (if records
          (dolist (record records)
            (org-slipbox-node--insert-link-occurrence record related-node-slot))
        (insert empty-message "\n")))
    (display-buffer (current-buffer))))

(defun org-slipbox-node--insert-link-occurrence (record related-node-slot)
  "Insert RECORD using RELATED-NODE-SLOT for the linked node payload."
  (let* ((related-node (plist-get record related-node-slot))
         (file (plist-get related-node :file_path)))
    (insert
     (org-slipbox--node-display related-node)
     "\n  "
     (format "%s:%s:%s"
             file
             (plist-get record :row)
             (plist-get record :col))
     " "
     (plist-get record :preview)
     "\n")))

(provide 'org-slipbox-node)

;;; org-slipbox-node.el ends here
