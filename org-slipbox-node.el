;;; org-slipbox-node.el --- Node commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Node indexing and lookup commands for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-search-limit 50
  "Maximum number of nodes to request for interactive search."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-capture-templates
  '(("d" "default" :path "${slug}.org" :title "${title}"))
  "Capture templates for `org-slipbox-capture'.
Each template is a list of the form (KEY DESCRIPTION [:path STRING] [:title STRING])."
  :type 'sexp
  :group 'org-slipbox)

(defun org-slipbox-index ()
  "Rebuild the local org-slipbox index from Org files."
  (interactive)
  (let* ((response (org-slipbox-rpc-request "slipbox/index"))
         (files (plist-get response :files_indexed))
         (nodes (plist-get response :nodes_indexed))
         (links (plist-get response :links_indexed)))
    (message "Indexed %s files, %s nodes, %s links" files nodes links)
    response))

(defun org-slipbox-capture (&optional title)
  "Create a new note with TITLE and visit it."
  (interactive)
  (let* ((title (or title (read-string "Capture title: ")))
         (template (org-slipbox--read-capture-template)))
    (org-slipbox--visit-node (org-slipbox--capture-node title template))))

(defun org-slipbox-node-find (query)
  "Find a node matching QUERY and visit it."
  (interactive (list (read-string "Find node: ")))
  (let ((node (org-slipbox--select-or-capture-node query)))
    (when node
      (org-slipbox--visit-node node))))

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
         (response (and node
                        (org-slipbox-rpc-request
                         "slipbox/backlinks"
                         `(:node_key ,node-key :limit 200))))
         (backlinks (and response (plist-get response :backlinks))))
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

(defun org-slipbox--select-or-capture-node (query)
  "Return a node selected for QUERY, or create one."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchNodes"
                    `(:query ,query :limit ,org-slipbox-search-limit)))
         (nodes (plist-get response :nodes))
         (create-choice (format "[Create] %s" query))
         (choices (mapcar (lambda (node)
                            (cons (org-slipbox--node-display node) node))
                          nodes))
         (collection (append choices (list (cons create-choice :create))))
         (selection (completing-read "Node: " collection nil t nil nil create-choice))
         (choice (cdr (assoc selection collection))))
    (cond
     ((eq choice :create) (org-slipbox--capture-node query))
     (choice choice)
     (t nil))))

(defun org-slipbox--capture-node (title &optional template)
  "Capture a new node with TITLE using TEMPLATE."
  (let* ((template (or template (org-slipbox--default-capture-template)))
         (template-options (and template (nthcdr 2 template)))
         (current-time (current-time))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             current-time)
                            title))
         (file-path (org-slipbox--expand-capture-template
                     (plist-get template-options :path)
                     title
                     current-time)))
    (org-slipbox-rpc-request
     "slipbox/captureNode"
     (if file-path
         `(:title ,capture-title :file_path ,file-path)
       `(:title ,capture-title)))))

(defun org-slipbox--ensure-node-id (node)
  "Return NODE with an explicit ID, assigning one if needed."
  (if (plist-get node :explicit_id)
      node
    (org-slipbox-rpc-request
     "slipbox/ensureNodeId"
     `(:node_key ,(plist-get node :node_key)))))

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

(defun org-slipbox--default-capture-template ()
  "Return the default capture template."
  (car org-slipbox-capture-templates))

(defun org-slipbox--read-capture-template ()
  "Prompt for a capture template when more than one is configured."
  (cond
   ((null org-slipbox-capture-templates) nil)
   ((= (length org-slipbox-capture-templates) 1)
    (car org-slipbox-capture-templates))
   (t
    (let* ((choices (mapcar (lambda (template)
                              (cons (format "%s %s" (car template) (cadr template))
                                    template))
                            org-slipbox-capture-templates))
           (selection (completing-read "Template: " choices nil t)))
      (cdr (assoc selection choices))))))

(defun org-slipbox--expand-capture-template (template title time)
  "Expand TEMPLATE for TITLE using TIME."
  (when template
    (let ((expanded (replace-regexp-in-string
                     "%<\\([^>]+\\)>"
                     (lambda (match)
                       (format-time-string
                        (substring match 2 -1)
                        time))
                     template
                     t)))
      (setq expanded
            (replace-regexp-in-string
             (regexp-quote "${title}")
             title
             expanded
             t
             t))
      (replace-regexp-in-string
       (regexp-quote "${slug}")
       (org-slipbox--slugify title)
       expanded
       t
       t))))

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

(defun org-slipbox--visit-node (node)
  "Visit NODE in its source file."
  (find-file (expand-file-name (plist-get node :file_path) org-slipbox-directory))
  (goto-char (point-min))
  (forward-line (1- (plist-get node :line))))

(provide 'org-slipbox-node)

;;; org-slipbox-node.el ends here
