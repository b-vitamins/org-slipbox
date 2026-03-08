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

(require 'org-slipbox-agenda)
(require 'org-slipbox-buffer)
(require 'org-slipbox-capture)
(require 'org-slipbox-discovery)
(require 'org-slipbox-edit)
(require 'org-slipbox-files)
(require 'org-slipbox-id)
(require 'org-slipbox-link)
(require 'org-slipbox-maintenance)
(require 'org-slipbox-metadata)
(require 'org-slipbox-mode)
(require 'org-slipbox-node)
(require 'org-slipbox-protocol)
(require 'org-slipbox-rpc)
(require 'org-slipbox-dailies)
(require 'org-slipbox-sync)

;;;###autoload
(defun org-slipbox-ping ()
  "Check that the local org-slipbox daemon responds."
  (interactive)
  (let* ((response (org-slipbox-rpc-ping))
         (version (plist-get response :version))
         (root (plist-get response :root)))
    (message "org-slipbox %s at %s" version root)
    response))

;;;###autoload(autoload 'org-slipbox-index "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-capture "org-slipbox-capture" nil t)
;;;###autoload(autoload 'org-slipbox-capture-ref "org-slipbox-capture" nil t)
;;;###autoload(autoload 'org-slipbox-capture-to-node "org-slipbox-capture" nil t)
;;;###autoload(autoload 'org-slipbox-file-p "org-slipbox-files" nil t)
;;;###autoload(autoload 'org-slipbox-list-files "org-slipbox-files" nil t)
;;;###autoload(autoload 'org-slipbox-id-mode "org-slipbox-id" nil t)
;;;###autoload(autoload 'org-slipbox-update-org-id-locations "org-slipbox-id" nil t)
;;;###autoload(autoload 'org-slipbox-mode "org-slipbox-mode" nil t)
;;;###autoload(autoload 'org-slipbox-sync "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-rebuild "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-sync-current-file "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-diagnose-node "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-diagnose-file "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-list-files-report "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-db-explore "org-slipbox-maintenance" nil t)
;;;###autoload(autoload 'org-slipbox-node-read "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-find "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-random "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-insert "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-node-backlinks "org-slipbox-node" nil t)
;;;###autoload(autoload 'org-slipbox-demote-entire-buffer "org-slipbox-edit" nil t)
;;;###autoload(autoload 'org-slipbox-promote-entire-buffer "org-slipbox-edit" nil t)
;;;###autoload(autoload 'org-slipbox-refile "org-slipbox-edit" nil t)
;;;###autoload(autoload 'org-slipbox-extract-subtree "org-slipbox-edit" nil t)
;;;###autoload(autoload 'org-slipbox-link-replace-all "org-slipbox-link" nil t)
;;;###autoload(autoload 'org-slipbox-completion-mode "org-slipbox-link" nil t)
;;;###autoload(autoload 'org-slipbox-export-mode "org-slipbox-export" nil t)
;;;###autoload(autoload 'org-slipbox-buffer-refresh "org-slipbox-buffer" nil t)
;;;###autoload(autoload 'org-slipbox-buffer-display-dedicated "org-slipbox-buffer" nil t)
;;;###autoload(autoload 'org-slipbox-buffer-toggle "org-slipbox-buffer" nil t)
;;;###autoload(autoload 'org-slipbox-graph "org-slipbox-graph" nil t)
;;;###autoload(autoload 'org-slipbox-graph-write-dot "org-slipbox-graph" nil t)
;;;###autoload(autoload 'org-slipbox-graph-write-file "org-slipbox-graph" nil t)
;;;###autoload(autoload 'org-slipbox-ref-read "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-ref-find "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-ref-add "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-ref-remove "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-alias-add "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-alias-remove "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-tag-add "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-tag-remove "org-slipbox-metadata" nil t)
;;;###autoload(autoload 'org-slipbox-agenda-today "org-slipbox-agenda" nil t)
;;;###autoload(autoload 'org-slipbox-agenda-date "org-slipbox-agenda" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-capture-today "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-today "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-capture-tomorrow "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-tomorrow "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-capture-yesterday "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-yesterday "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-capture-date "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-date "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-next-note "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-goto-previous-note "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-find-directory "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-calendar-mark-entries "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-dailies-calendar-mode "org-slipbox-dailies" nil t)
;;;###autoload(autoload 'org-slipbox-protocol-mode "org-slipbox-protocol" nil t)
;;;###autoload(autoload 'org-slipbox-autosync-mode "org-slipbox-sync" nil t)

(provide 'org-slipbox)

;;; org-slipbox.el ends here
