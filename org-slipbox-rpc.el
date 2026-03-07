;;; org-slipbox-rpc.el --- JSON-RPC client for org-slipbox -*- lexical-binding: t; -*-

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

;; Internal JSON-RPC transport helpers for `org-slipbox'.

;;; Code:

(require 'jsonrpc)
(require 'subr-x)

(defgroup org-slipbox nil
  "Local-first Org knowledge tools."
  :group 'applications
  :prefix "org-slipbox-")

(defcustom org-slipbox-server-program "slipbox"
  "Path to the org-slipbox server executable."
  :type 'file
  :group 'org-slipbox)

(defcustom org-slipbox-directory nil
  "Root directory containing Org files for org-slipbox."
  :type 'directory
  :group 'org-slipbox)

(defcustom org-slipbox-database-file
  (expand-file-name "org-slipbox.sqlite" user-emacs-directory)
  "Path to the local org-slipbox SQLite database."
  :type 'file
  :group 'org-slipbox)

(defvar org-slipbox--connection nil
  "Live JSON-RPC connection to the local org-slipbox process.")

(defun org-slipbox-rpc-live-p ()
  "Return non-nil when the org-slipbox JSON-RPC process is live."
  (and org-slipbox--connection
       (jsonrpc-running-p org-slipbox--connection)))

(defun org-slipbox-rpc-ensure ()
  "Start and return the org-slipbox JSON-RPC connection."
  (unless (file-directory-p org-slipbox-directory)
    (user-error "`org-slipbox-directory' must name an existing directory"))
  (unless (org-slipbox-rpc-live-p)
    (setq org-slipbox--connection
          (make-instance
           'jsonrpc-process-connection
           :name "org-slipbox"
           :events-buffer-config '(:size 200 :format full)
           :process (lambda ()
                      (make-process
                       :name "org-slipbox"
                       :command (list org-slipbox-server-program
                                      "serve"
                                      "--root" (expand-file-name org-slipbox-directory)
                                      "--db" (expand-file-name org-slipbox-database-file))
                       :connection-type 'pipe
                       :coding 'binary
                       :noquery t
                       :stderr (get-buffer-create "*org-slipbox stderr*")))
           :notification-dispatcher #'ignore
           :request-dispatcher #'ignore
           :on-shutdown (lambda (_conn)
                          (setq org-slipbox--connection nil)))))
  org-slipbox--connection)

(defun org-slipbox-rpc-request (method &optional params)
  "Send METHOD with PARAMS to the local org-slipbox daemon."
  (jsonrpc-request (org-slipbox-rpc-ensure) method params))

(provide 'org-slipbox-rpc)

;;; org-slipbox-rpc.el ends here
