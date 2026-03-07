;;; org-slipbox.el --- Local-first Org slipbox tools -*- lexical-binding: t; -*-

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

;; org-slipbox provides a local-first Org knowledge workflow backed by a
;; dedicated index and query engine.  This file contains the package entry
;; points and user-facing commands.

;;; Code:

(require 'org-slipbox-node)
(require 'org-slipbox-rpc)
(require 'org-slipbox-sync)

;;;###autoload
(defun org-slipbox-ping ()
  "Check that the local org-slipbox daemon responds."
  (interactive)
  (let* ((response (org-slipbox-rpc-request "slipbox/ping"))
         (version (plist-get response :version))
         (root (plist-get response :root)))
    (message "org-slipbox %s at %s" version root)
    response))

;;;###autoload(autoload 'org-slipbox-index "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-capture "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-find "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-insert "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-backlinks "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-autosync-mode "org-slipbox-sync" nil t)

(provide 'org-slipbox)

;;; org-slipbox.el ends here
