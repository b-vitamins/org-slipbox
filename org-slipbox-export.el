;;; org-slipbox-export.el --- Export support for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.6.1
;; Package-Requires: ((emacs "29.1") (org "9.6"))
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

;; Optional HTML export support for `id:' links that target Org IDs.

;;; Code:

(require 'ox-html)

(defun org-slipbox-export--org-html-reference (datum info &optional named-only)
  "Return a stable HTML reference for DATUM, INFO, and NAMED-ONLY.
This keeps Org ID-based targets aligned with the `id:' link export path."
  (let* ((type (org-element-type datum))
         (label
          (org-element-property
           (pcase type
             ((or `headline `inlinetask) :CUSTOM_ID)
             ((or `radio-target `target) :value)
             (_ :name))
           datum))
         (label
          (or label
              (when-let ((identifier (org-element-property :ID datum)))
                (concat "ID-" identifier)))))
    (cond
     ((and label
           (or (plist-get info :html-prefer-user-labels)
               (memq type '(headline inlinetask))))
      label)
     ((and named-only
           (not (memq type '(headline inlinetask radio-target target)))
           (not label))
      nil)
     (t
      (org-export-get-reference datum info)))))

;;;###autoload
(define-minor-mode org-slipbox-export-mode
  "Toggle HTML export support for Org ID-backed links."
  :global t
  :group 'org-slipbox
  (if org-slipbox-export-mode
      (advice-add 'org-html--reference
                  :override
                  #'org-slipbox-export--org-html-reference)
    (advice-remove 'org-html--reference
                   #'org-slipbox-export--org-html-reference)))

(provide 'org-slipbox-export)

;;; org-slipbox-export.el ends here
