;;; org-slipbox-rpc.el --- JSON-RPC client for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.9.0
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

(require 'org-slipbox-discovery)
(require 'jsonrpc)

(defcustom org-slipbox-server-program "slipbox"
  "Path or program name of the org-slipbox server executable.

When this names a bare program such as `slipbox', org-slipbox
resolves it through `exec-path'."
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

(defvar org-slipbox--connection-config nil
  "Configuration used to start `org-slipbox--connection'.")

(defconst org-slipbox-rpc-method-ping "slipbox/ping")
(defconst org-slipbox-rpc-method-status "slipbox/status")
(defconst org-slipbox-rpc-method-index "slipbox/index")
(defconst org-slipbox-rpc-method-index-file "slipbox/indexFile")
(defconst org-slipbox-rpc-method-indexed-files "slipbox/indexedFiles")
(defconst org-slipbox-rpc-method-search-files "slipbox/searchFiles")
(defconst org-slipbox-rpc-method-search-occurrences "slipbox/searchOccurrences")
(defconst org-slipbox-rpc-method-graph-dot "slipbox/graphDot")
(defconst org-slipbox-rpc-method-search-nodes "slipbox/searchNodes")
(defconst org-slipbox-rpc-method-random-node "slipbox/randomNode")
(defconst org-slipbox-rpc-method-search-tags "slipbox/searchTags")
(defconst org-slipbox-rpc-method-node-from-id "slipbox/nodeFromId")
(defconst org-slipbox-rpc-method-node-from-title-or-alias "slipbox/nodeFromTitleOrAlias")
(defconst org-slipbox-rpc-method-node-from-ref "slipbox/nodeFromRef")
(defconst org-slipbox-rpc-method-node-at-point "slipbox/nodeAtPoint")
(defconst org-slipbox-rpc-method-anchor-at-point "slipbox/anchorAtPoint")
(defconst org-slipbox-rpc-method-backlinks "slipbox/backlinks")
(defconst org-slipbox-rpc-method-forward-links "slipbox/forwardLinks")
(defconst org-slipbox-rpc-method-reflinks "slipbox/reflinks")
(defconst org-slipbox-rpc-method-unlinked-references "slipbox/unlinkedReferences")
(defconst org-slipbox-rpc-method-explore "slipbox/explore")
(defconst org-slipbox-rpc-method-compare-notes "slipbox/compareNotes")
(defconst org-slipbox-rpc-method-save-exploration-artifact "slipbox/saveExplorationArtifact")
(defconst org-slipbox-rpc-method-exploration-artifact "slipbox/explorationArtifact")
(defconst org-slipbox-rpc-method-list-exploration-artifacts "slipbox/listExplorationArtifacts")
(defconst org-slipbox-rpc-method-execute-exploration-artifact "slipbox/executeExplorationArtifact")
(defconst org-slipbox-rpc-method-agenda "slipbox/agenda")
(defconst org-slipbox-rpc-method-search-refs "slipbox/searchRefs")
(defconst org-slipbox-rpc-method-capture-node "slipbox/captureNode")
(defconst org-slipbox-rpc-method-capture-template "slipbox/captureTemplate")
(defconst org-slipbox-rpc-method-capture-template-preview "slipbox/captureTemplatePreview")
(defconst org-slipbox-rpc-method-ensure-file-node "slipbox/ensureFileNode")
(defconst org-slipbox-rpc-method-append-heading "slipbox/appendHeading")
(defconst org-slipbox-rpc-method-append-heading-to-node "slipbox/appendHeadingToNode")
(defconst org-slipbox-rpc-method-append-heading-at-outline-path "slipbox/appendHeadingAtOutlinePath")
(defconst org-slipbox-rpc-method-ensure-node-id "slipbox/ensureNodeId")
(defconst org-slipbox-rpc-method-update-node-metadata "slipbox/updateNodeMetadata")
(defconst org-slipbox-rpc-method-refile-subtree "slipbox/refileSubtree")
(defconst org-slipbox-rpc-method-refile-region "slipbox/refileRegion")
(defconst org-slipbox-rpc-method-extract-subtree "slipbox/extractSubtree")
(defconst org-slipbox-rpc-method-promote-entire-file "slipbox/promoteEntireFile")
(defconst org-slipbox-rpc-method-demote-entire-file "slipbox/demoteEntireFile")

(defun org-slipbox-rpc--bool (value)
  "Return VALUE encoded as an explicit JSON boolean."
  (if value t :json-false))

(defun org-slipbox-rpc--json-plist-p (value)
  "Return non-nil when VALUE is a keyword plist."
  (and (listp value)
       (catch 'invalid
         (let ((tail value))
           (while tail
             (unless (keywordp (pop tail))
               (throw 'invalid nil))
             (unless tail
               (throw 'invalid nil))
             (pop tail))
           t))))

(defun org-slipbox-rpc--json-alist-p (value)
  "Return non-nil when VALUE is a JSON-style alist."
  (and (listp value)
       value
       (catch 'invalid
         (dolist (entry value t)
           (unless (and (consp entry)
                        (let ((key (car entry)))
                          (or (keywordp key)
                              (stringp key)
                              (symbolp key))))
             (throw 'invalid nil))))))

(defun org-slipbox-rpc--json-normalize (value)
  "Normalize VALUE into a shape accepted by `json-serialize'."
  (cond
   ((vectorp value)
    (apply #'vector (mapcar #'org-slipbox-rpc--json-normalize value)))
   ((org-slipbox-rpc--json-plist-p value)
    (let (normalized)
      (while value
        (let ((key (pop value))
              (nested (pop value)))
          (setq normalized
                (append normalized
                        (list key
                              (org-slipbox-rpc--json-normalize nested))))))
      normalized))
   ((org-slipbox-rpc--json-alist-p value)
    (mapcar (lambda (entry)
              (cons (car entry)
                    (org-slipbox-rpc--json-normalize (cdr entry))))
            value))
   ((listp value)
    (apply #'vector (mapcar #'org-slipbox-rpc--json-normalize value)))
   (t value)))

(defun org-slipbox--plist-sequence (value)
  "Normalize JSON-derived VALUE into an Emacs list."
  (cond
   ((null value) nil)
   ((vectorp value) (append value nil))
   ((listp value) value)
   (t (list value))))

(defun org-slipbox-rpc-live-p ()
  "Return non-nil when the org-slipbox JSON-RPC process is live."
  (and org-slipbox--connection
       (jsonrpc-running-p org-slipbox--connection)))

(defun org-slipbox-rpc--resolve-server-program ()
  "Return the resolved daemon executable path for the current configuration."
  (let* ((program (string-trim (or org-slipbox-server-program "")))
         (path-like (and (not (string-empty-p program))
                         (or (file-name-absolute-p program)
                             (file-name-directory program))))
         (resolved (cond
                    ((string-empty-p program)
                     (user-error
                      "`org-slipbox-server-program' must name a slipbox executable"))
                    (path-like
                     (expand-file-name program))
                    (t
                     (or (executable-find program)
                         (user-error
                          (concat "Cannot find the slipbox daemon `%s'. "
                                  "Put `slipbox' on PATH or set "
                                  "`org-slipbox-server-program' to the binary path.")
                          program))))))
    (unless (file-exists-p resolved)
      (user-error "Cannot find the slipbox daemon at %s" resolved))
    (unless (file-executable-p resolved)
      (user-error "The slipbox daemon at %s is not executable" resolved))
    resolved))

(defun org-slipbox-rpc--command ()
  "Return the daemon command for the current configuration."
  (append
   (list (org-slipbox-rpc--resolve-server-program)
         "serve"
         "--root" (expand-file-name org-slipbox-directory)
         "--db" (expand-file-name org-slipbox-database-file))
   (org-slipbox-discovery-command-args)))

(defun org-slipbox-rpc--connection-config ()
  "Return the normalized connection configuration."
  (list :program (org-slipbox-rpc--resolve-server-program)
        :root (expand-file-name org-slipbox-directory)
        :db (expand-file-name org-slipbox-database-file)
        :file-extensions (org-slipbox-discovery-file-extensions)
        :exclude-regexp (org-slipbox-discovery-exclude-regexps)))

(defun org-slipbox-rpc-ensure ()
  "Start and return the org-slipbox JSON-RPC connection."
  (unless (file-directory-p org-slipbox-directory)
    (user-error "`org-slipbox-directory' must name an existing directory"))
  (let ((config (org-slipbox-rpc--connection-config)))
    (when (and (org-slipbox-rpc-live-p)
               (not (equal config org-slipbox--connection-config)))
      (jsonrpc-shutdown org-slipbox--connection)
      (setq org-slipbox--connection nil))
    (unless (org-slipbox-rpc-live-p)
      (setq org-slipbox--connection
            (make-instance
             'jsonrpc-process-connection
             :name "org-slipbox"
             :events-buffer-config '(:size 200 :format full)
             :process (lambda ()
                        (make-process
                         :name "org-slipbox"
                         :command (org-slipbox-rpc--command)
                         :connection-type 'pipe
                         :coding 'binary
                         :noquery t
                         :stderr (get-buffer-create "*org-slipbox stderr*")))
             :notification-dispatcher #'ignore
             :request-dispatcher #'ignore
             :on-shutdown (lambda (_conn)
                            (setq org-slipbox--connection nil
                                  org-slipbox--connection-config nil))))
      (setq org-slipbox--connection-config config)))
  org-slipbox--connection)

(defun org-slipbox-rpc-request (method &optional params)
  "Send METHOD with PARAMS to the local org-slipbox daemon."
  (jsonrpc-request (org-slipbox-rpc-ensure)
                   method
                   (org-slipbox-rpc--json-normalize params)))

(defun org-slipbox-rpc-reset ()
  "Shutdown the live org-slipbox JSON-RPC connection, if any."
  (when org-slipbox--connection
    (let ((connection org-slipbox--connection))
      (setq org-slipbox--connection nil
            org-slipbox--connection-config nil)
      (when (jsonrpc-running-p connection)
        (jsonrpc-shutdown connection)))))

(defun org-slipbox-rpc-ping ()
  "Request daemon identity information."
  (org-slipbox-rpc-request org-slipbox-rpc-method-ping))

(defun org-slipbox-rpc-status ()
  "Request daemon and index status information."
  (org-slipbox-rpc-request org-slipbox-rpc-method-status))

(defun org-slipbox-rpc-index ()
  "Rebuild the index for the configured slipbox root."
  (org-slipbox-rpc-request org-slipbox-rpc-method-index))

(defun org-slipbox-rpc-index-file (file-path)
  "Sync FILE-PATH into the index."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-index-file
   `(:file_path ,(expand-file-name file-path))))

(defun org-slipbox-rpc-indexed-files ()
  "Return the relative paths currently stored in the index."
  (org-slipbox-rpc-request org-slipbox-rpc-method-indexed-files))

(defun org-slipbox-rpc-search-files (query limit)
  "Search indexed files matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-files
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-search-occurrences (query limit)
  "Search indexed text occurrences matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-occurrences
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-graph-dot (params)
  "Return Graphviz DOT for graph generation PARAMS."
  (org-slipbox-rpc-request org-slipbox-rpc-method-graph-dot params))

(defun org-slipbox-rpc--search-node-sort-name (sort)
  "Return SORT encoded for the `searchNodes' RPC surface."
  (cond
   ((null sort) nil)
   ((member sort '("relevance"
                   "title"
                   "file"
                   "file-mtime"
                   "backlink-count"
                   "forward-link-count"))
    sort)
   ((eq sort 'relevance) "relevance")
   ((eq sort 'title) "title")
   ((eq sort 'file) "file")
   ((eq sort 'file-mtime) "file-mtime")
   ((eq sort 'backlink-count) "backlink-count")
   ((eq sort 'forward-link-count) "forward-link-count")
   (t
    (user-error "Unsupported searchNodes sort %s" sort))))

(defun org-slipbox-rpc--exploration-lens-name (lens)
  "Return LENS encoded for the `explore' RPC surface."
  (cond
   ((member lens '("structure" "refs" "time" "tasks" "bridges" "dormant" "unresolved"))
    lens)
   ((eq lens 'structure) "structure")
   ((eq lens 'refs) "refs")
   ((eq lens 'time) "time")
   ((eq lens 'tasks) "tasks")
   ((eq lens 'bridges) "bridges")
   ((eq lens 'dormant) "dormant")
   ((eq lens 'unresolved) "unresolved")
   (t
    (user-error "Unsupported explore lens %s" lens))))

(defun org-slipbox-rpc-search-nodes (query limit &optional sort)
  "Search canonical nodes matching QUERY with LIMIT and optional SORT."
  (let ((params `(:query ,query :limit ,limit)))
    (when-let ((sort-name (org-slipbox-rpc--search-node-sort-name sort)))
      (setq params (append params `(:sort ,sort-name))))
    (org-slipbox-rpc-request
     org-slipbox-rpc-method-search-nodes
     params)))

(defun org-slipbox-rpc-random-node ()
  "Return a random canonical node."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-random-node))

(defun org-slipbox-rpc-search-tags (query limit)
  "Search indexed tags matching QUERY with LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-search-tags
   `(:query ,query :limit ,limit)))

(defun org-slipbox-rpc-node-from-id (id)
  "Resolve the canonical node identified by ID."
  (org-slipbox-rpc-request org-slipbox-rpc-method-node-from-id `(:id ,id)))

(defun org-slipbox-rpc-node-from-title-or-alias (title-or-alias &optional nocase)
  "Resolve a node by TITLE-OR-ALIAS.
When NOCASE is non-nil, use case-insensitive matching."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-from-title-or-alias
   `(:title_or_alias ,title-or-alias
                     :nocase ,(org-slipbox-rpc--bool nocase))))

(defun org-slipbox-rpc-node-from-ref (reference)
  "Resolve a node by REFERENCE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-from-ref
   `(:reference ,reference)))

(defun org-slipbox-rpc-node-at-point (file-path line)
  "Resolve the canonical node at FILE-PATH and LINE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-node-at-point
   `(:file_path ,(expand-file-name file-path) :line ,line)))

(defun org-slipbox-rpc-anchor-at-point (file-path line)
  "Resolve the indexed anchor at FILE-PATH and LINE."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-anchor-at-point
   `(:file_path ,(expand-file-name file-path) :line ,line)))

(defun org-slipbox-rpc-backlinks (node-key &optional limit unique)
  "Return backlinks for NODE-KEY, optionally capped by LIMIT.
When UNIQUE is non-nil, only return the first backlink occurrence
per source node."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-backlinks
   `(:node_key ,node-key :limit ,(or limit 200)
               :unique ,(org-slipbox-rpc--bool unique))))

(defun org-slipbox-rpc-forward-links (node-key &optional limit unique)
  "Return forward links for NODE-KEY, optionally capped by LIMIT.
When UNIQUE is non-nil, only return the first forward-link occurrence
per destination node."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-forward-links
   `(:node_key ,node-key :limit ,(or limit 200)
               :unique ,(org-slipbox-rpc--bool unique))))

(defun org-slipbox-rpc-explore (node-key lens &optional limit unique)
  "Explore NODE-KEY through declared LENS semantics.
LIMIT bounds the number of rows requested per section. When UNIQUE is
non-nil, structure-lens occurrences collapse to one row per related note."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-explore
   `(:node_key ,node-key
               :lens ,(org-slipbox-rpc--exploration-lens-name lens)
               :limit ,(or limit 200)
               :unique ,(org-slipbox-rpc--bool unique))))

(defun org-slipbox-rpc-compare-notes (left-node-key right-node-key &optional limit)
  "Compare LEFT-NODE-KEY with RIGHT-NODE-KEY through the daemon."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-compare-notes
   `(:left_node_key ,left-node-key
                    :right_node_key ,right-node-key
                    :limit ,(or limit 200))))

(defun org-slipbox-rpc-save-exploration-artifact (artifact)
  "Persist saved exploration ARTIFACT through the daemon."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-save-exploration-artifact
   `(:artifact ,artifact)))

(defun org-slipbox-rpc-exploration-artifact (artifact-id)
  "Load the saved exploration artifact identified by ARTIFACT-ID."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-exploration-artifact
   `(:artifact_id ,artifact-id)))

(defun org-slipbox-rpc-list-exploration-artifacts ()
  "List saved exploration artifact summaries."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-list-exploration-artifacts))

(defun org-slipbox-rpc-execute-exploration-artifact (artifact-id)
  "Execute the saved exploration artifact identified by ARTIFACT-ID."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-execute-exploration-artifact
   `(:artifact_id ,artifact-id)))

(defun org-slipbox-rpc-reflinks (node-key &optional limit)
  "Return reflinks for NODE-KEY, optionally capped by LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-reflinks
   `(:node_key ,node-key :limit ,(or limit 200))))

(defun org-slipbox-rpc-unlinked-references (node-key &optional limit)
  "Return unlinked references for NODE-KEY, optionally capped by LIMIT."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-unlinked-references
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

(defun org-slipbox-rpc-capture-template-preview (params)
  "Preview capture using generic template PARAMS without saving."
  (org-slipbox-rpc-request org-slipbox-rpc-method-capture-template-preview params))

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

(defun org-slipbox-rpc-refile-region (file-path start end target-node-key)
  "Refile the region from FILE-PATH between START and END under TARGET-NODE-KEY."
  (org-slipbox-rpc-request
   org-slipbox-rpc-method-refile-region
   `(:file_path ,(expand-file-name file-path)
     :start ,start
     :end ,end
     :target_node_key ,target-node-key)))

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
