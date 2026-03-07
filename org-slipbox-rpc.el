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

(defconst org-slipbox-rpc-method-ping "slipbox/ping")
(defconst org-slipbox-rpc-method-index "slipbox/index")
(defconst org-slipbox-rpc-method-index-file "slipbox/indexFile")
(defconst org-slipbox-rpc-method-search-nodes "slipbox/searchNodes")
(defconst org-slipbox-rpc-method-random-node "slipbox/randomNode")
(defconst org-slipbox-rpc-method-search-tags "slipbox/searchTags")
(defconst org-slipbox-rpc-method-node-from-id "slipbox/nodeFromId")
(defconst org-slipbox-rpc-method-node-from-title-or-alias "slipbox/nodeFromTitleOrAlias")
(defconst org-slipbox-rpc-method-node-from-ref "slipbox/nodeFromRef")
(defconst org-slipbox-rpc-method-node-at-point "slipbox/nodeAtPoint")
(defconst org-slipbox-rpc-method-backlinks "slipbox/backlinks")
(defconst org-slipbox-rpc-method-agenda "slipbox/agenda")
(defconst org-slipbox-rpc-method-search-refs "slipbox/searchRefs")
(defconst org-slipbox-rpc-method-capture-node "slipbox/captureNode")
(defconst org-slipbox-rpc-method-capture-template "slipbox/captureTemplate")
(defconst org-slipbox-rpc-method-ensure-file-node "slipbox/ensureFileNode")
(defconst org-slipbox-rpc-method-append-heading "slipbox/appendHeading")
(defconst org-slipbox-rpc-method-append-heading-to-node "slipbox/appendHeadingToNode")
(defconst org-slipbox-rpc-method-append-heading-at-outline-path "slipbox/appendHeadingAtOutlinePath")
(defconst org-slipbox-rpc-method-ensure-node-id "slipbox/ensureNodeId")
(defconst org-slipbox-rpc-method-update-node-metadata "slipbox/updateNodeMetadata")
(defconst org-slipbox-rpc-method-refile-subtree "slipbox/refileSubtree")
(defconst org-slipbox-rpc-method-extract-subtree "slipbox/extractSubtree")
(defconst org-slipbox-rpc-method-promote-entire-file "slipbox/promoteEntireFile")
(defconst org-slipbox-rpc-method-demote-entire-file "slipbox/demoteEntireFile")

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

(defun org-slipbox-rpc-ping ()
  "Request daemon identity information."
  (org-slipbox-rpc-request org-slipbox-rpc-method-ping))

(defun org-slipbox-rpc-index ()
  "Rebuild the index for the configured slipbox root."
  (org-slipbox-rpc-request org-slipbox-rpc-method-index))

(defun org-slipbox-rpc-index-file (file-path)
  "Sync FILE-PATH into the index."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-index-file
   `(:file_path ,(expand-file-name file-path))))

(defun org-slipbox-rpc-search-nodes (query limit)
  "Search nodes matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-nodes
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-random-node ()
  "Return a random indexed node."
  (org-slipbox-rpc-request org-slipbox-rpc-method-random-node))

(defun org-slipbox-rpc-search-tags (query limit)
  "Search indexed tags matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-tags
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-node-from-id (id)
  "Resolve the indexed node identified by ID."
  (org-slipbox-rpc-request org-slipbox-rpc-method-node-from-id `(:id ,id)))

(defun org-slipbox-rpc-node-from-title-or-alias (title-or-alias &optional nocase)
  "Resolve a node by TITLE-OR-ALIAS.
When NOCASE is non-nil, use case-insensitive matching."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-from-title-or-alias
   `(:title_or_alias ,title-or-alias :nocase ,(and nocase t))))

(defun org-slipbox-rpc-node-from-ref (reference)
  "Resolve a node by REFERENCE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-from-ref
   `(:reference ,reference)))

(defun org-slipbox-rpc-node-at-point (file-path line)
  "Resolve the indexed node at FILE-PATH and LINE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-at-point
   `(:file_path ,(expand-file-name file-path) :line ,line)))

(defun org-slipbox-rpc-backlinks (node-key &optional limit)
  "Return backlinks for NODE-KEY, optionally capped by LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-backlinks
   `(:node_key ,node-key :limit ,(or limit 200))))

(defun org-slipbox-rpc-agenda (start end)
  "Return indexed agenda entries between START and END."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-agenda
   `(:start ,start :end ,end)))

(defun org-slipbox-rpc-search-refs (query limit)
  "Search references matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-refs
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-capture-node (params)
  "Capture a new node using PARAMS."
  (org-slipbox-rpc-request org-slipbox-rpc-method-capture-node params))

(defun org-slipbox-rpc-capture-template (params)
  "Capture using generic template PARAMS."
  (org-slipbox-rpc-request org-slipbox-rpc-method-capture-template params))

(defun org-slipbox-rpc-ensure-file-node (file-path title)
  "Ensure FILE-PATH exists as a file node with TITLE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-ensure-file-node
   `(:file_path ,file-path :title ,title)))

(defun org-slipbox-rpc-append-heading (file-path title heading level)
  "Append HEADING at LEVEL into FILE-PATH with file TITLE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-append-heading
   `(:file_path ,file-path :title ,title :heading ,heading :level ,level)))

(defun org-slipbox-rpc-append-heading-to-node (node-key heading)
  "Append HEADING under NODE-KEY."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-append-heading-to-node
   `(:node_key ,node-key :heading ,heading)))

(defun org-slipbox-rpc-append-heading-at-outline-path (params)
  "Append a heading using outline-path capture PARAMS."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-append-heading-at-outline-path
   params))

(defun org-slipbox-rpc-ensure-node-id (node-key)
  "Ensure the node identified by NODE-KEY has an explicit Org ID."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-ensure-node-id
   `(:node_key ,node-key)))

(defun org-slipbox-rpc-update-node-metadata (params)
  "Update node metadata using PARAMS."
  (org-slipbox-rpc-request org-slipbox-rpc-method-update-node-metadata params))

(defun org-slipbox-rpc-refile-subtree (source-node-key target-node-key)
  "Refile SOURCE-NODE-KEY under TARGET-NODE-KEY."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-refile-subtree
   `(:source_node_key ,source-node-key :target_node_key ,target-node-key)))

(defun org-slipbox-rpc-extract-subtree (source-node-key file-path)
  "Extract SOURCE-NODE-KEY into FILE-PATH."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-extract-subtree
   `(:source_node_key ,source-node-key :file_path ,(expand-file-name file-path))))

(defun org-slipbox-rpc-promote-entire-file (file-path)
  "Promote FILE-PATH from a single root heading into a file node."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-promote-entire-file
   `(:file_path ,(expand-file-name file-path))))

(defun org-slipbox-rpc-demote-entire-file (file-path)
  "Demote FILE-PATH into a single root heading node."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-demote-entire-file
   `(:file_path ,(expand-file-name file-path))))

(provide 'org-slipbox-rpc)

;;; org-slipbox-rpc.el ends here
