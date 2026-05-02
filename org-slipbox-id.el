;;; org-slipbox-id.el --- org-id compatibility for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.6.1
;; Package-Requires: ((emacs "29.1") (jsonrpc "1.0.27") (org "9.6"))
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

;; Compatibility helpers that bridge `org-id' through the org-slipbox index
;; without letting `org-id-locations' override indexed truth.

;;; Code:

(require 'org-id)
(require 'subr-x)
(require 'org-slipbox-files)
(require 'org-slipbox-node)

;;;###autoload
(define-minor-mode org-slipbox-id-mode
  "Bridge `org-id' lookup through the org-slipbox index.

When enabled, `org-id-find' first consults the indexed org-slipbox
state.  If no indexed node matches, normal `org-id' lookup continues
unchanged."
  :global t
  :group 'org-slipbox
  (if org-slipbox-id-mode
      (advice-add 'org-id-find :before-until #'org-slipbox-id-find)
    (advice-remove 'org-id-find #'org-slipbox-id-find)))

;;;###autoload
(defun org-slipbox-id-find (id &optional markerp)
  "Return the indexed location of ID, or nil when it is unknown.

The return value matches `org-id-find': a cons cell of the form
\(FILE . POSITION), or a marker when MARKERP is non-nil."
  (when-let* ((id (org-slipbox-id--normalize id))
              ((file-directory-p org-slipbox-directory))
              (node (condition-case nil
                        (org-slipbox-node-from-id id)
                      (error nil)))
              (file-path (plist-get node :file_path)))
    (org-slipbox-id--location
     (expand-file-name file-path org-slipbox-directory)
     (or (plist-get node :line) 1)
     markerp)))

(defalias 'org-slipbox-id-open 'org-id-open
  "Compatibility alias; use `org-id-open' directly.")

;;;###autoload
(defun org-slipbox-update-org-id-locations (&rest paths)
  "Refresh `org-id-locations' for org-slipbox files and extra PATHS.

This uses the configured slipbox file extensions, but intentionally
ignores discovery exclusion regexps so valid `id:' targets outside the
index still cooperate with `org-id'.  Directory PATHS are scanned
recursively.  Explicit file PATHS are included when their extension is
supported."
  (interactive)
  (let ((files (org-slipbox--org-id-update-files paths)))
    (org-id-update-id-locations files nil)))

(defun org-slipbox-id--location (file line markerp)
  "Return FILE and LINE as an `org-id-find' style location.

When MARKERP is non-nil, return a fresh marker."
  (when (file-exists-p file)
    (if markerp
        (let ((buffer (or (find-buffer-visiting file)
                          (find-file-noselect file))))
          (with-current-buffer buffer
            (move-marker (make-marker)
                         (org-slipbox-id--line-position (or line 1))
                         buffer)))
      (with-temp-buffer
        (insert-file-contents file)
        (cons file (org-slipbox-id--line-position (or line 1)))))))

(defun org-slipbox-id--line-position (line)
  "Return the point at LINE in the current buffer."
  (goto-char (point-min))
  (forward-line (max 0 (1- line)))
  (point))

(defun org-slipbox-id--normalize (id)
  "Normalize ID into a string, or nil when it is unusable."
  (cond
   ((symbolp id) (symbol-name id))
   ((numberp id) (number-to-string id))
   ((stringp id) id)
   (t nil)))

(defun org-slipbox--org-id-update-files (paths)
  "Return supported files from org-slipbox roots and extra PATHS."
  (let ((roots (delete-dups
                (delq nil
                      (mapcar #'org-slipbox--existing-directory
                              (list org-slipbox-directory org-directory))))))
    (delete-dups
     (append (apply #'append (mapcar #'org-slipbox--list-supported-files roots))
             (apply #'append (mapcar #'org-slipbox--org-id-path-files paths))))))

(defun org-slipbox--org-id-path-files (path)
  "Return supported files from explicit PATH."
  (let ((path (expand-file-name path)))
    (cond
     ((file-directory-p path)
      (org-slipbox--list-supported-files path))
     ((file-regular-p path)
      (unless (and (file-readable-p path)
                   (org-slipbox--supported-file-p path))
        (user-error "Unsupported org-id compatibility file: %s" path))
      (list path))
     (t
      (user-error "No such org-id compatibility path: %s" path)))))

(defun org-slipbox--existing-directory (path)
  "Return PATH as an expanded directory name when it exists."
  (when (and path (file-directory-p path))
    (file-name-as-directory (expand-file-name path))))

(provide 'org-slipbox-id)

;;; org-slipbox-id.el ends here
