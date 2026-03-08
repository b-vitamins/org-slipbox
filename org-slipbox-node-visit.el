;;; org-slipbox-node-visit.el --- Node visit helpers for org-slipbox -*- lexical-binding: t; -*-

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

;; Buffer coordination and visit helpers for indexed nodes.

;;; Code:

(require 'seq)
(require 'org-slipbox-files)
(require 'org-slipbox-rpc)

(defvar org-slipbox-directory)

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
         (org-slipbox-file-p file))))

(defun org-slipbox--current-node-buffer-file ()
  "Return the current base buffer file path."
  (buffer-file-name (or (buffer-base-buffer) (current-buffer))))

(defun org-slipbox--visit-node (node &optional other-window)
  "Visit NODE in its source file.
With OTHER-WINDOW, visit it in another window."
  (funcall (if other-window #'find-file-other-window #'find-file)
           (expand-file-name (plist-get node :file_path) org-slipbox-directory))
  (goto-char (point-min))
  (forward-line (1- (plist-get node :line))))

(provide 'org-slipbox-node-visit)

;;; org-slipbox-node-visit.el ends here
