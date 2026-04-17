;;; org-slipbox-mode.el --- Integration mode for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.3.0
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

;; Integration mode for `org-slipbox'.

;;; Code:

(require 'org-slipbox-files)
(require 'org-slipbox-id)
(require 'org-slipbox-link)
(require 'org-slipbox-sync)

(defvar org-slipbox-directory)

(defvar org-slipbox-mode--managed-autosync nil
  "Non-nil when `org-slipbox-mode' enabled `org-slipbox-autosync-mode'.")

(defvar org-slipbox-mode--managed-id-bridge nil
  "Non-nil when `org-slipbox-mode' enabled `org-slipbox-id-mode'.")

(defvar-local org-slipbox-mode--managed-completion nil
  "Non-nil when `org-slipbox-mode' enabled completion in this buffer.")

(define-minor-mode org-slipbox-mode
  "Enable the single-mode `org-slipbox' integration surface.

This explicit global mode turns on the `org-id' bridge,
incremental autosync, and buffer-local completion in eligible Org
files under `org-slipbox-directory'. Loading `org-slipbox' alone
does not enable any of these behaviors."
  :global t
  :group 'org-slipbox
  (if org-slipbox-mode
      (org-slipbox-mode--enable)
    (org-slipbox-mode--disable)))

(defun org-slipbox-mode--enable ()
  "Enable the single-mode org-slipbox integration surface."
  (unless org-slipbox-autosync-mode
    (org-slipbox-autosync-mode 1)
    (setq org-slipbox-mode--managed-autosync t))
  (unless org-slipbox-id-mode
    (org-slipbox-id-mode 1)
    (setq org-slipbox-mode--managed-id-bridge t))
  (add-hook 'find-file-hook #'org-slipbox-mode--maybe-enable-completion)
  (dolist (buffer (buffer-list))
    (org-slipbox-mode--maybe-enable-completion buffer)))

(defun org-slipbox-mode--disable ()
  "Disable the single-mode org-slipbox integration surface."
  (remove-hook 'find-file-hook #'org-slipbox-mode--maybe-enable-completion)
  (dolist (buffer (buffer-list))
    (org-slipbox-mode--disable-managed-completion buffer))
  (when org-slipbox-mode--managed-id-bridge
    (setq org-slipbox-mode--managed-id-bridge nil)
    (when org-slipbox-id-mode
      (org-slipbox-id-mode -1)))
  (when org-slipbox-mode--managed-autosync
    (setq org-slipbox-mode--managed-autosync nil)
    (when org-slipbox-autosync-mode
      (org-slipbox-autosync-mode -1))))

(defun org-slipbox-mode--maybe-enable-completion (&optional buffer)
  "Enable completion in eligible BUFFER when `org-slipbox-mode' is active."
  (with-current-buffer (or buffer (current-buffer))
    (when (org-slipbox-mode--eligible-org-buffer-p)
      (unless org-slipbox-completion-mode
        (org-slipbox-completion-mode 1)
        (setq-local org-slipbox-mode--managed-completion t)))))

(defun org-slipbox-mode--disable-managed-completion (&optional buffer)
  "Disable completion in BUFFER when it was enabled by `org-slipbox-mode'."
  (with-current-buffer (or buffer (current-buffer))
    (when org-slipbox-mode--managed-completion
      (setq-local org-slipbox-mode--managed-completion nil)
      (when org-slipbox-completion-mode
        (org-slipbox-completion-mode -1)))))

(defun org-slipbox-mode--eligible-org-buffer-p ()
  "Return non-nil when the current buffer should get completion."
  (let* ((buffer (or (buffer-base-buffer) (current-buffer)))
         (file (buffer-file-name buffer)))
    (and file
         (derived-mode-p 'org-mode)
         org-slipbox-directory
         (org-slipbox-file-p file))))

(provide 'org-slipbox-mode)

;;; org-slipbox-mode.el ends here
