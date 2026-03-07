;;; org-slipbox-edit.el --- Structural editing commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Structural subtree editing commands for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-capture)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-extract-file-name-template "${slug}.org"
  "Default relative file target template for `org-slipbox-extract-subtree'."
  :type 'string
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-demote-entire-buffer ()
  "Demote the current Org buffer into a single root heading node."
  (interactive)
  (let ((file (org-slipbox--current-edit-file)))
    (org-slipbox--sync-live-file-buffer-if-needed file)
    (prog1
        (org-slipbox-rpc-demote-entire-file file)
      (org-slipbox--refresh-or-kill-file-buffer file))))

;;;###autoload
(defun org-slipbox-promote-entire-buffer ()
  "Promote the current Org buffer from a single root heading into a file node."
  (interactive)
  (let ((file (org-slipbox--current-edit-file)))
    (org-slipbox--sync-live-file-buffer-if-needed file)
    (prog1
        (org-slipbox-rpc-promote-entire-file file)
      (org-slipbox--refresh-or-kill-file-buffer file))))

;;;###autoload
(defun org-slipbox-refile (node)
  "Refile the current subtree under NODE."
  (interactive (list (org-slipbox--read-existing-node "Refile to: ")))
  (unless node
    (user-error "No target node selected"))
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (let* ((source-node (org-slipbox-node-at-point t))
         (source-file (expand-file-name (plist-get source-node :file_path)
                                        org-slipbox-directory))
         (target-file (expand-file-name (plist-get node :file_path)
                                        org-slipbox-directory))
         moved-node)
    (when (equal (plist-get source-node :node_key)
                 (plist-get node :node_key))
      (user-error "Target is the same as current node"))
    (org-slipbox--sync-live-file-buffer-if-needed target-file)
    (setq moved-node
          (org-slipbox-rpc-refile-subtree
           (plist-get source-node :node_key)
           (plist-get node :node_key)))
    (org-slipbox--refresh-or-kill-file-buffer source-file)
    (unless (equal source-file target-file)
      (org-slipbox--refresh-or-kill-file-buffer target-file))
    moved-node))

;;;###autoload
(defun org-slipbox-extract-subtree (&optional file-path)
  "Extract the current subtree into FILE-PATH under `org-slipbox-directory'."
  (interactive)
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (let* ((source-node (org-slipbox-node-at-point t))
         (source-file (expand-file-name (plist-get source-node :file_path)
                                        org-slipbox-directory))
         (target-path (org-slipbox--extract-target-path
                       (plist-get source-node :title)
                       file-path)))
    (when (equal (plist-get source-node :kind) "file")
      (user-error "Already a top-level node"))
    (org-slipbox-rpc-extract-subtree (plist-get source-node :node_key) target-path)
    (org-slipbox--refresh-or-kill-file-buffer source-file)
    (org-slipbox--refresh-or-kill-file-buffer target-path)
    target-path))

(defun org-slipbox--current-edit-file ()
  "Return the current file path for a structural edit command."
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  buffer-file-name)

(defun org-slipbox--refresh-or-kill-file-buffer (path)
  "Refresh the live buffer visiting PATH, or kill it when PATH no longer exists."
  (if (file-exists-p path)
      (org-slipbox--refresh-live-file-buffer path)
    (org-slipbox--kill-live-file-buffer path)))

(defun org-slipbox--extract-target-path (title &optional file-path)
  "Return an absolute extraction path for TITLE and FILE-PATH."
  (let* ((suggested (org-slipbox--expand-capture-template
                     org-slipbox-extract-file-name-template
                     title
                     (current-time)))
         (path (or file-path
                   (read-file-name
                    "Extract node to: "
                    (file-name-as-directory org-slipbox-directory)
                    suggested
                    nil
                    suggested)))
         (absolute (expand-file-name path org-slipbox-directory)))
    (unless (file-in-directory-p absolute (expand-file-name org-slipbox-directory))
      (user-error "%s is not inside org-slipbox-directory" absolute))
    (unless (string-suffix-p ".org" absolute)
      (user-error "Extracted file must end with .org"))
    absolute))

(provide 'org-slipbox-edit)

;;; org-slipbox-edit.el ends here
