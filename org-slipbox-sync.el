;;; org-slipbox-sync.el --- Save-driven sync for org-slipbox -*- lexical-binding: t; -*-

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

;; Save-driven synchronization helpers for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-rpc)

(define-minor-mode org-slipbox-autosync-mode
  "Keep the org-slipbox index current for saved Org buffers."
  :global t
  :group 'org-slipbox
  (if org-slipbox-autosync-mode
      (add-hook 'after-save-hook #'org-slipbox-sync-current-buffer)
    (remove-hook 'after-save-hook #'org-slipbox-sync-current-buffer)))

(defun org-slipbox-sync-current-buffer ()
  "Sync the current buffer file into the org-slipbox index."
  (interactive)
  (when (org-slipbox--syncable-buffer-p)
    (condition-case error
        (org-slipbox-rpc-index-file buffer-file-name)
      (error
       (message "org-slipbox sync failed: %s" (error-message-string error))))))

(defun org-slipbox--syncable-buffer-p ()
  "Return non-nil when the current buffer should be synced."
  (and buffer-file-name
       org-slipbox-directory
       (string-suffix-p ".org" buffer-file-name)
       (file-in-directory-p (expand-file-name buffer-file-name)
                            (expand-file-name org-slipbox-directory))))

(provide 'org-slipbox-sync)

;;; org-slipbox-sync.el ends here
