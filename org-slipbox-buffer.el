;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

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

;; Context buffer commands for `org-slipbox'.

;;; Code:

(require 'button)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defvar org-slipbox-buffer "*org-slipbox*"
  "Name of the persistent org-slipbox context buffer.")

(defvar-local org-slipbox-buffer-current-node nil
  "Node currently rendered in the org-slipbox context buffer.")

(define-derived-mode org-slipbox-buffer-mode special-mode "org-slipbox"
  "Major mode for org-slipbox context buffers.")

;;;###autoload
(defun org-slipbox-buffer-refresh ()
  "Refresh the current org-slipbox context buffer."
  (interactive)
  (unless (derived-mode-p 'org-slipbox-buffer-mode)
    (user-error "Not in an org-slipbox buffer"))
  (unless org-slipbox-buffer-current-node
    (user-error "No org-slipbox node to refresh"))
  (org-slipbox-buffer-render-contents))

;;;###autoload
(defun org-slipbox-buffer-display-dedicated (node)
  "Display a dedicated org-slipbox buffer for NODE."
  (interactive (list (org-slipbox-buffer--read-node-for-display)))
  (let ((buffer (get-buffer-create (org-slipbox-buffer--dedicated-name node))))
    (with-current-buffer buffer
      (setq-local org-slipbox-buffer-current-node node)
      (org-slipbox-buffer-render-contents))
    (display-buffer buffer)))

;;;###autoload
(defun org-slipbox-buffer-toggle ()
  "Toggle display of the persistent org-slipbox context buffer."
  (interactive)
  (if (get-buffer-window org-slipbox-buffer 'visible)
      (progn
        (quit-window nil (get-buffer-window org-slipbox-buffer))
        (remove-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h))
    (display-buffer (get-buffer-create org-slipbox-buffer))
    (org-slipbox-buffer-persistent-redisplay)
    (add-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)))

(defun org-slipbox-buffer-persistent-redisplay ()
  "Refresh the persistent org-slipbox context buffer from point."
  (when-let ((node (org-slipbox-node-at-point)))
    (with-current-buffer (get-buffer-create org-slipbox-buffer)
      (unless (equal node org-slipbox-buffer-current-node)
        (setq-local org-slipbox-buffer-current-node node)
        (org-slipbox-buffer-render-contents)
        (add-hook 'kill-buffer-hook #'org-slipbox-buffer--persistent-cleanup-h nil t)))))

(defun org-slipbox-buffer-render-contents ()
  "Render the current org-slipbox context buffer."
  (let* ((node org-slipbox-buffer-current-node)
         (backlinks (and node (org-slipbox-buffer--backlinks node)))
         (inhibit-read-only t))
    (erase-buffer)
    (org-slipbox-buffer-mode)
    (setq-local header-line-format
                (when node
                  (concat (propertize " " 'display '(space :align-to 0))
                          (plist-get node :title))))
    (when node
      (org-slipbox-buffer--insert-heading (plist-get node :title))
      (org-slipbox-buffer--insert-metadata-line "File" (plist-get node :file_path))
      (when-let ((outline (plist-get node :outline_path)))
        (unless (string-empty-p outline)
          (org-slipbox-buffer--insert-metadata-line "Outline" outline)))
      (when-let ((explicit-id (plist-get node :explicit_id)))
        (org-slipbox-buffer--insert-metadata-line "ID" explicit-id))
      (when-let ((aliases (org-slipbox--plist-sequence (plist-get node :aliases))))
        (when aliases
          (org-slipbox-buffer--insert-metadata-line "Aliases" (string-join aliases ", "))))
      (when-let ((tags (org-slipbox--plist-sequence (plist-get node :tags))))
        (when tags
          (org-slipbox-buffer--insert-metadata-line "Tags" (string-join tags ", "))))
      (insert "\n")
      (org-slipbox-buffer--insert-ref-section node)
      (org-slipbox-buffer--insert-backlink-section backlinks))
    (goto-char (point-min))))

(defun org-slipbox-buffer--redisplay-h ()
  "Keep the persistent org-slipbox context buffer in sync with point."
  (when (and (get-buffer-window org-slipbox-buffer 'visible)
             (not (buffer-modified-p (or (buffer-base-buffer) (current-buffer)))))
    (org-slipbox-buffer-persistent-redisplay)))

(defun org-slipbox-buffer--persistent-cleanup-h ()
  "Clean up persistent buffer global state."
  (when (string= (buffer-name) org-slipbox-buffer)
    (remove-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)))

(defun org-slipbox-buffer--dedicated-name (node)
  "Return a dedicated context buffer name for NODE."
  (format "*org-slipbox: %s<%s>*"
          (plist-get node :title)
          (plist-get node :file_path)))

(defun org-slipbox-buffer--read-node-for-display ()
  "Read a node for dedicated buffer display."
  (or (org-slipbox-node-at-point)
      (let ((query (read-string "Node: ")))
        (or (org-slipbox-node-from-title-or-alias query)
            (let* ((response (org-slipbox-rpc-request
                              "slipbox/searchNodes"
                              `(:query ,query :limit ,org-slipbox-search-limit)))
                   (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
                   (choices (mapcar (lambda (candidate)
                                      (cons (org-slipbox--node-display candidate) candidate))
                                    nodes))
                   (selection (and choices
                                   (completing-read "Node: " choices nil t))))
              (and selection (cdr (assoc selection choices))))))))

(defun org-slipbox-buffer--backlinks (node)
  "Return backlinks for NODE."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/backlinks"
                    `(:node_key ,(plist-get node :node_key) :limit 200)))
         (backlinks (plist-get response :backlinks)))
    (org-slipbox--plist-sequence backlinks)))

(defun org-slipbox-buffer--insert-heading (text)
  "Insert section heading TEXT."
  (insert text "\n")
  (insert (make-string (length text) ?=) "\n\n"))

(defun org-slipbox-buffer--insert-metadata-line (label value)
  "Insert LABEL and VALUE on one line."
  (insert (propertize (format "%-7s " (concat label ":")) 'face 'bold)
          value
          "\n"))

(defun org-slipbox-buffer--insert-ref-section (node)
  "Insert a refs section for NODE."
  (let ((refs (org-slipbox--plist-sequence (plist-get node :refs))))
    (when refs
      (insert "Refs\n----\n")
      (dolist (reference refs)
        (insert reference "\n"))
      (insert "\n"))))

(defun org-slipbox-buffer--insert-backlink-section (backlinks)
  "Insert a backlinks section for BACKLINKS."
  (insert "Backlinks\n---------\n")
  (if backlinks
      (dolist (backlink backlinks)
        (org-slipbox-buffer--insert-node-button backlink)
        (insert "\n"))
    (insert "No backlinks found.\n")))

(defun org-slipbox-buffer--insert-node-button (node)
  "Insert a button for NODE."
  (insert-text-button
   (org-slipbox--node-display node)
   'follow-link t
   'help-echo "Visit node"
   'action (lambda (_button)
             (org-slipbox--visit-node node))))

(provide 'org-slipbox-buffer)

;;; org-slipbox-buffer.el ends here
