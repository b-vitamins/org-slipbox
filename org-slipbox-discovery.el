;;; org-slipbox-discovery.el --- Discovery policy for org-slipbox -*- lexical-binding: t; -*-

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

;; Shared file-discovery policy helpers for `org-slipbox'.

;;; Code:

(require 'subr-x)

(defgroup org-slipbox nil
  "Local-first Org knowledge tools."
  :group 'applications
  :prefix "org-slipbox-")

(defcustom org-slipbox-file-extensions '("org")
  "File extensions eligible for discovery and indexing.

These extensions are matched case-insensitively. Encrypted files
with an outer `.gpg' or `.age' suffix remain eligible when their base
extension matches this list."
  :type '(repeat string)
  :group 'org-slipbox)

(defcustom org-slipbox-file-exclude-regexp nil
  "Relative-path regexp or regexps excluded from discovery.

When this is a string, it is applied as one regexp. When it is a
list, each element must be a regexp string."
  :type '(choice
          (const :tag "No exclusions" nil)
          (regexp :tag "One regexp")
          (repeat :tag "Regexp list" regexp))
  :group 'org-slipbox)

(defun org-slipbox-discovery-file-extensions ()
  "Return normalized discovery extensions."
  (or (delete-dups
       (delq nil
             (mapcar
              (lambda (extension)
                (let ((extension (string-trim (or extension ""))))
                  (unless (string-empty-p extension)
                    (downcase (string-remove-prefix "." extension)))))
              org-slipbox-file-extensions)))
      '("org")))

(defun org-slipbox-discovery-exclude-regexps ()
  "Return normalized discovery exclusion regexps."
  (let ((patterns (cond
                   ((null org-slipbox-file-exclude-regexp) nil)
                   ((stringp org-slipbox-file-exclude-regexp)
                    (list org-slipbox-file-exclude-regexp))
                   ((listp org-slipbox-file-exclude-regexp)
                    org-slipbox-file-exclude-regexp)
                   (t
                    (user-error
                     "`org-slipbox-file-exclude-regexp' must be nil, a string, or a list of strings")))))
    (delq nil
          (mapcar
           (lambda (pattern)
             (let ((pattern (string-trim (or pattern ""))))
               (unless (string-empty-p pattern)
                 pattern)))
           patterns))))

(defun org-slipbox-discovery-command-args ()
  "Return daemon CLI arguments for the current discovery policy."
  (append
   (apply #'append
          (mapcar (lambda (extension)
                    (list "--file-extension" extension))
                  (org-slipbox-discovery-file-extensions)))
   (apply #'append
          (mapcar (lambda (pattern)
                    (list "--exclude-regexp" pattern))
                  (org-slipbox-discovery-exclude-regexps)))))

(provide 'org-slipbox-discovery)

;;; org-slipbox-discovery.el ends here
