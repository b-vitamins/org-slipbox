;;; org-slipbox-files.el --- File discovery policy for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.8.0
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

;; Public file-discovery helpers for `org-slipbox'.

;;; Code:

(require 'seq)
(require 'subr-x)
(require 'org-slipbox-discovery)
(require 'org-slipbox-rpc)

(defvar org-slipbox-directory)

;;;###autoload
(defun org-slipbox-file-p (file &optional root)
  "Return non-nil when FILE is eligible under the current discovery policy.

When ROOT is non-nil, evaluate eligibility relative to that root.
Otherwise use `org-slipbox-directory'."
  (when-let* ((root (or root org-slipbox-directory))
              (expanded-root (file-name-as-directory (expand-file-name root)))
              (expanded-file (expand-file-name file))
              ((file-in-directory-p expanded-file expanded-root))
              (relative-path (file-relative-name expanded-file expanded-root))
              (extension (org-slipbox--file-base-extension expanded-file)))
    (and (member extension (org-slipbox-discovery-file-extensions))
         (not (org-slipbox--excluded-relative-path-p relative-path)))))

;;;###autoload
(defun org-slipbox-list-files (&optional root)
  "Return eligible files under ROOT, or `org-slipbox-directory' when nil."
  (when-let* ((root (or root org-slipbox-directory))
              (expanded-root (file-name-as-directory (expand-file-name root)))
              ((file-directory-p expanded-root)))
    (sort
     (seq-filter
      (lambda (file)
        (org-slipbox-file-p file expanded-root))
      (org-slipbox--list-supported-files expanded-root))
     #'string-lessp)))

;;;###autoload
(defun org-slipbox-search-files (query &optional limit)
  "Return indexed file records matching QUERY.
LIMIT defaults to 200 when omitted."
  (org-slipbox--plist-sequence
   (plist-get (org-slipbox-rpc-search-files query (or limit 200)) :files)))

(defun org-slipbox--file-recursive-regexp ()
  "Return the recursive listing regexp for eligible files."
  (format "\\.%s\\(?:\\.gpg\\|\\.age\\)?\\'"
          (regexp-opt (org-slipbox-discovery-file-extensions))))

(defun org-slipbox--file-base-extension (file)
  "Return FILE's base extension, stripping outer encrypted suffixes."
  (let ((extension (downcase (or (file-name-extension file) ""))))
    (cond
     ((member extension '("gpg" "age"))
      (when-let ((inner (file-name-sans-extension file)))
        (org-slipbox--file-base-extension inner)))
     ((string-empty-p extension) nil)
     (t extension))))

(defun org-slipbox--supported-file-p (file)
  "Return non-nil when FILE has a supported base extension."
  (when-let ((extension (org-slipbox--file-base-extension file)))
    (member extension (org-slipbox-discovery-file-extensions))))

(defun org-slipbox--file-name-stem (file)
  "Return FILE's stem, stripping one encrypted suffix when present."
  (let* ((outer-extension (downcase (or (file-name-extension file) "")))
         (candidate (if (member outer-extension '("gpg" "age"))
                        (file-name-sans-extension file)
                      file)))
    (file-name-sans-extension (file-name-nondirectory candidate))))

(defun org-slipbox--excluded-relative-path-p (relative-path)
  "Return non-nil when RELATIVE-PATH is excluded by the current policy."
  (seq-some
   (lambda (pattern)
     (string-match-p pattern relative-path))
   (org-slipbox-discovery-exclude-regexps)))

(defun org-slipbox--list-supported-files (root)
  "Return supported files under ROOT, ignoring exclusion regexps."
  (let ((case-fold-search t)
        (expanded-root (file-name-as-directory (expand-file-name root))))
    (seq-filter
     (lambda (file)
       (and (file-regular-p file)
            (file-readable-p file)
            (org-slipbox--supported-file-p file)))
     (directory-files-recursively expanded-root (org-slipbox--file-recursive-regexp)))))

(provide 'org-slipbox-files)

;;; org-slipbox-files.el ends here
