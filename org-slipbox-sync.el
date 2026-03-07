;;; org-slipbox-sync.el --- Autosync for org-slipbox file lifecycle -*- lexical-binding: t; -*-

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

;; Explicit autosync helpers for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-rpc)

(declare-function vc-delete-file "vc" (&optional file))

(define-minor-mode org-slipbox-autosync-mode
  "Keep the org-slipbox index current for Org file lifecycle events."
  :global t
  :group 'org-slipbox
  (if org-slipbox-autosync-mode
      (org-slipbox--autosync-enable)
    (org-slipbox--autosync-disable)))

(defun org-slipbox--autosync-enable ()
  "Enable autosync hooks and advices."
  (add-hook 'find-file-hook #'org-slipbox--autosync-setup-file-h)
  (advice-add #'rename-file :around #'org-slipbox--autosync-rename-file-a)
  (advice-add #'delete-file :around #'org-slipbox--autosync-delete-file-a)
  (advice-add #'vc-delete-file :around #'org-slipbox--autosync-vc-delete-file-a)
  (dolist (buffer (buffer-list))
    (with-current-buffer buffer
      (org-slipbox--autosync-setup-buffer))))

(defun org-slipbox--autosync-disable ()
  "Disable autosync hooks and advices."
  (remove-hook 'find-file-hook #'org-slipbox--autosync-setup-file-h)
  (advice-remove #'rename-file #'org-slipbox--autosync-rename-file-a)
  (advice-remove #'delete-file #'org-slipbox--autosync-delete-file-a)
  (advice-remove #'vc-delete-file #'org-slipbox--autosync-vc-delete-file-a)
  (dolist (buffer (buffer-list))
    (with-current-buffer buffer
      (remove-hook 'after-save-hook #'org-slipbox-sync-current-buffer t))))

(defun org-slipbox--autosync-setup-file-h ()
  "Configure autosync for the current file-visiting buffer."
  (org-slipbox--autosync-setup-buffer))

(defun org-slipbox--autosync-setup-buffer ()
  "Install buffer-local autosync hooks when the current buffer is tracked."
  (if (org-slipbox--syncable-buffer-p)
      (add-hook 'after-save-hook #'org-slipbox-sync-current-buffer nil t)
    (remove-hook 'after-save-hook #'org-slipbox-sync-current-buffer t)))

(defun org-slipbox-sync-current-buffer ()
  "Sync the current buffer file into the org-slipbox index."
  (interactive)
  (when (org-slipbox--syncable-buffer-p)
    (org-slipbox--autosync-sync-file buffer-file-name "save")))

(defun org-slipbox--syncable-buffer-p ()
  "Return non-nil when the current buffer should be synced."
  (org-slipbox--syncable-file-p buffer-file-name))

(defun org-slipbox--syncable-file-p (file)
  "Return non-nil when FILE belongs to the configured slipbox root."
  (and file
       org-slipbox-directory
       (string-suffix-p ".org" file)
       (not (auto-save-file-name-p file))
       (not (backup-file-name-p file))
       (let ((expanded-file (expand-file-name file))
             (expanded-root (file-name-as-directory
                             (expand-file-name org-slipbox-directory))))
         (string-prefix-p expanded-root expanded-file))))

(defun org-slipbox--autosync-sync-file (file context)
  "Sync FILE into the index for lifecycle CONTEXT."
  (condition-case error
      (org-slipbox-rpc-index-file file)
    (error
     (message
      "org-slipbox autosync %s failed for %s: %s"
      context
      (abbreviate-file-name (expand-file-name file))
      (error-message-string error)))))

(defun org-slipbox--autosync-delete-file-a (function file &rest args)
  "Maintain index correctness when FUNCTION deletes FILE."
  (let ((tracked (org-slipbox--syncable-file-p file))
        (expanded-file (expand-file-name file)))
    (prog1 (apply function file args)
      (when (and tracked (not (file-exists-p expanded-file)))
        (org-slipbox--autosync-sync-file expanded-file "delete")))))

(defun org-slipbox--autosync-vc-delete-file-a (function file &rest args)
  "Maintain index correctness when FUNCTION deletes FILE through VC."
  (let ((tracked (org-slipbox--syncable-file-p file))
        (expanded-file (expand-file-name file)))
    (prog1 (apply function file args)
      (when (and tracked (not (file-exists-p expanded-file)))
        (org-slipbox--autosync-sync-file expanded-file "vc-delete")))))

(defun org-slipbox--autosync-rename-file-a (function old-file new-file-or-dir &rest args)
  "Maintain index correctness when FUNCTION renames OLD-FILE to NEW-FILE-OR-DIR."
  (let* ((old-file (expand-file-name old-file))
         (new-file (org-slipbox--autosync-rename-target old-file new-file-or-dir))
         (tracked-old (org-slipbox--syncable-file-p old-file))
         (tracked-new (org-slipbox--syncable-file-p new-file)))
    (prog1 (apply function old-file new-file-or-dir args)
      (when tracked-old
        (org-slipbox--autosync-sync-file old-file "rename"))
      (when tracked-new
        (org-slipbox--autosync-sync-file new-file "rename")))))

(defun org-slipbox--autosync-rename-target (old-file new-file-or-dir)
  "Resolve OLD-FILE renamed to NEW-FILE-OR-DIR into the final file path."
  (expand-file-name
   (if (or (directory-name-p new-file-or-dir)
           (file-directory-p new-file-or-dir))
       (expand-file-name (file-name-nondirectory old-file) new-file-or-dir)
     new-file-or-dir)))

(provide 'org-slipbox-sync)

;;; org-slipbox-sync.el ends here
