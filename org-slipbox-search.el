;;; org-slipbox-search.el --- Search helpers for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.1.0
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

;; Public search helpers for `org-slipbox'.

;;; Code:

(require 'org-slipbox-rpc)

;;;###autoload
(defun org-slipbox-search-occurrences (query &optional limit)
  "Return indexed text occurrences matching QUERY.
Queries shorter than 3 characters after trimming surrounding whitespace
return no hits.
LIMIT defaults to 200 when omitted."
  (org-slipbox--plist-sequence
   (plist-get (org-slipbox-rpc-search-occurrences query (or limit 200))
              :occurrences)))

(provide 'org-slipbox-search)

;;; org-slipbox-search.el ends here
