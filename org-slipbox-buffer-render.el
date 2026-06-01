;;; org-slipbox-buffer-render.el --- Rendering helpers for org-slipbox context buffers -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.14.3
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

;; Rendering helpers for org-slipbox context buffers.

;;; Code:

(require 'button)
(require 'cl-lib)
(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-buffer-query)
(require 'org-slipbox-buffer-state)
(require 'org-slipbox-files)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defun org-slipbox-buffer-render-contents ()
  "Render the current org-slipbox context buffer."
  (let* ((node (org-slipbox-buffer--session-node))
         (inhibit-read-only t))
    (erase-buffer)
    (org-slipbox-buffer-mode)
    (setq-local header-line-format (org-slipbox-buffer--header-line node))
    (when (org-slipbox-buffer--trail-active-p)
      (org-slipbox-buffer--render-trail-section))
    (when node
      (if (org-slipbox-buffer--comparison-active-p)
          (org-slipbox-buffer--render-comparison node)
        (org-slipbox-buffer--render-sections node)))
    (run-hooks 'org-slipbox-buffer-postrender-functions)
    (goto-char (point-min))))

(defun org-slipbox-buffer--render-sections (node)
  "Render the active section plan for NODE."
  (dolist (section (org-slipbox-buffer--current-section-plan))
    (when (org-slipbox-buffer--section-allowed-p section node)
      (org-slipbox-buffer--render-section section node))))

(defun org-slipbox-buffer--section-allowed-p (section node)
  "Return non-nil when SECTION should render for NODE."
  (or (null org-slipbox-buffer-section-filter-function)
      (funcall org-slipbox-buffer-section-filter-function section node)))

(defun org-slipbox-buffer--render-section (section node)
  "Render SECTION for NODE."
  (pcase section
    ((pred functionp)
     (funcall section node))
    (`(,fn . ,args)
     (unless (functionp fn)
       (user-error "Invalid org-slipbox buffer section function: %S" fn))
     (apply fn node args))
    (_
     (user-error "Invalid org-slipbox buffer section specification: %S" section))))

(defun org-slipbox-buffer--redisplay-h ()
  "Keep the persistent org-slipbox context buffer in sync with point."
  (when (and (get-buffer-window org-slipbox-buffer 'visible)
             (not (buffer-modified-p (or (buffer-base-buffer) (current-buffer)))))
    (org-slipbox-buffer-persistent-redisplay)))

(defun org-slipbox-buffer--persistent-cleanup-h ()
  "Clean up persistent buffer global state."
  (when (string= (buffer-name) org-slipbox-buffer)
    (org-slipbox-buffer-persistent-mode -1)))

(defun org-slipbox-buffer--header-line (node)
  "Return the header-line display for NODE."
  (when node
    (let ((parts (list (plist-get node :title))))
      (when (org-slipbox-buffer--dedicated-p)
        (if-let ((compare-target (org-slipbox-buffer--compare-target)))
            (setq parts
                  (append
                   parts
                   (list
                    (format "compare: %s" (plist-get compare-target :title))
                    (format "group: %s"
                            (symbol-name
                             (org-slipbox-buffer--current-comparison-group))))))
          (when-let ((lens (org-slipbox-buffer--current-lens)))
            (setq parts
                  (append parts
                          (list (format "lens: %s" (symbol-name lens)))))))
        (when-let ((session org-slipbox-buffer-session))
          (when (and (org-slipbox-buffer-session-frozen-context session)
                     (not (equal node (org-slipbox-buffer-session-root-node session))))
            (setq parts
                  (append
                   parts
                   (list
                    (format "root: %s"
                            (plist-get
                             (org-slipbox-buffer-session-root-node session)
                             :title)))))))
        (when-let ((position (org-slipbox-buffer--trail-position)))
          (setq parts
                (append
                 parts
                 (list
                  (format "trail: %s/%s%s"
                          (1+ position)
                          (length (org-slipbox-buffer--trail))
                          (if (org-slipbox-buffer--trail-attached-p) "" "*")))))))
      (concat (propertize " " 'display '(space :align-to 0))
              (string-join parts "  |  ")))))

(defun org-slipbox-buffer--transition-dedicated (snapshot)
  "Apply dedicated-buffer SNAPSHOT as a navigable transition."
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (current (org-slipbox-buffer--history-snapshot session)))
    (unless (equal current snapshot)
      (setf (org-slipbox-buffer-session-history session)
            (cons current (org-slipbox-buffer-session-history session))
            (org-slipbox-buffer-session-future session) nil)
      (org-slipbox-buffer--apply-history-snapshot session snapshot)
      (org-slipbox-buffer--reconcile-trail-position session)
      (org-slipbox-buffer-render-contents))))

(defun org-slipbox-buffer--replay-trail-at (index)
  "Replay the explicit trail step at INDEX."
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (trail (org-slipbox-buffer-session-trail session))
         (snapshot (nth index trail)))
    (unless snapshot
      (user-error "No trail step at index %s" index))
    (setf (org-slipbox-buffer-session-trail-index session) index)
    (if (equal snapshot (org-slipbox-buffer--history-snapshot session))
        (org-slipbox-buffer-render-contents)
      (org-slipbox-buffer--transition-dedicated snapshot))))

(defun org-slipbox-buffer--require-dedicated-session ()
  "Return the active dedicated buffer session, or signal a user error."
  (unless (org-slipbox-buffer--dedicated-p)
    (user-error "This command is only available in dedicated org-slipbox buffers"))
  org-slipbox-buffer-session)

(defun org-slipbox-buffer--dedicated-name (node)
  "Return a dedicated context buffer name for NODE."
  (format "*org-slipbox: %s<%s>*"
          (plist-get node :title)
          (plist-get node :file_path)))

(defun org-slipbox-buffer--dedicated-p (&optional buffer)
  "Return non-nil when BUFFER is a dedicated org-slipbox buffer."
  (with-current-buffer (or buffer (current-buffer))
    (and (org-slipbox-buffer-session-p org-slipbox-buffer-session)
         (eq (org-slipbox-buffer-session-kind org-slipbox-buffer-session)
             'dedicated))))

(defun org-slipbox-buffer--render-expensive-sections-p ()
  "Return non-nil when expensive discovery sections should be rendered."
  (pcase org-slipbox-buffer-expensive-sections
    ('always t)
    ('dedicated (org-slipbox-buffer--dedicated-p))
    (_ nil)))

(defun org-slipbox-buffer--read-node-for-display ()
  "Read a node for dedicated buffer display."
  (or (org-slipbox-node-at-point)
      (let ((query (read-string "Node: ")))
        (or (org-slipbox-node-from-title-or-alias query)
            (let* ((response (org-slipbox-rpc-search-nodes
                              query
                              org-slipbox-search-limit))
                   (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
                   (choices (mapcar (lambda (candidate)
                                      (cons (org-slipbox--node-display candidate) candidate))
                                   nodes))
                   (selection (and choices
                                   (completing-read "Node: " choices nil t))))
              (and selection (cdr (assoc selection choices))))))))

(defun org-slipbox-buffer--render-comparison (node)
  "Render dedicated comparison mode for NODE."
  (let* ((compare-target (org-slipbox-buffer--compare-target))
         (comparison (and compare-target
                          (org-slipbox-buffer--comparison-result node compare-target))))
    (org-slipbox-buffer-node-section
     (or (plist-get comparison :left_note) node)
     :heading "Current Note")
    (when compare-target
      (org-slipbox-buffer-node-section
       (or (plist-get comparison :right_note) compare-target)
       :heading "Compare Target"))
    (when comparison
      (org-slipbox-buffer--render-comparison-sections comparison))))

(defun org-slipbox-buffer--render-trail-section ()
  "Render the explicit exploratory trail for the current dedicated buffer."
  (org-slipbox-buffer--insert-heading "Trail")
  (dolist (line (org-slipbox-buffer--trail-status-lines))
    (insert (propertize line 'face 'italic) "\n"))
  (insert "\n")
  (dolist (entry (org-slipbox-buffer--trail-entries))
    (org-slipbox-buffer--insert-trail-entry entry)
    (insert "\n"))
  (insert "\n"))

(defun org-slipbox-buffer--trail-status-lines (&optional session)
  "Return user-facing trail status lines for SESSION or the current buffer."
  (let* ((session (or session org-slipbox-buffer-session))
         (trail (org-slipbox-buffer--trail session))
         (count (length trail))
         (position (org-slipbox-buffer--trail-position session)))
    (cond
     ((org-slipbox-buffer--trail-attached-p session)
      (list (format "status: attached at step %s of %s"
                    (1+ position)
                    count)))
     ((org-slipbox-buffer--trail-detached-p session)
      (list (format "status: detached from step %s of %s"
                    (1+ position)
                    count)
            "branch: current cockpit state is not yet recorded"))
     (t
      (list "status: no active trail")))))

(defun org-slipbox-buffer--trail-entries ()
  "Return decorated entries for the explicit trail."
  (let ((trail (org-slipbox-buffer--trail))
        (trail-index (org-slipbox-buffer--trail-position))
        (trail-attached (org-slipbox-buffer--trail-attached-p))
        (index 0)
        entries)
    (dolist (snapshot trail)
      (push (list :index index
                  :snapshot snapshot
                  :current (and trail-attached (eq index trail-index))
                  :branch-base (and (not trail-attached)
                                    (eq index trail-index)))
            entries)
      (setq index (1+ index)))
    (when (and trail
               (not trail-attached))
      (push (list :candidate t
                  :from-index trail-index
                  :snapshot (org-slipbox-buffer--history-snapshot))
            entries))
    (nreverse entries)))

(defun org-slipbox-buffer--insert-trail-entry (entry)
  "Insert one explicit trail ENTRY."
  (if (plist-get entry :candidate)
      (let* ((snapshot (plist-get entry :snapshot))
             (label (org-slipbox-buffer--trail-label snapshot))
             (from-index (plist-get entry :from-index)))
        (insert "~> ")
        (insert (format "current. %s" label))
        (when from-index
          (insert " "
                  (propertize
                   (format "[branch from step %s]" (1+ from-index))
                   'face 'shadow))))
    (let* ((index (plist-get entry :index))
           (snapshot (plist-get entry :snapshot))
           (label (org-slipbox-buffer--trail-label snapshot))
           (prefix (cond
                    ((plist-get entry :current) "=> ")
                    ((plist-get entry :branch-base) "|> ")
                    (t "   "))))
      (insert prefix)
      (insert-text-button
       (format "%s. %s" (1+ index) label)
       'follow-link t
       'help-echo "Replay this trail step"
       'action (lambda (_button)
                 (org-slipbox-buffer--replay-trail-at index)))
      (when (plist-get entry :branch-base)
        (insert " " (propertize "[branch base]" 'face 'shadow))))))

(defun org-slipbox-buffer--trail-label (snapshot)
  "Return a short label for trail SNAPSHOT."
  (let* ((node (plist-get snapshot :current-node))
         (root-node (plist-get snapshot :root-node))
         (compare-target (plist-get snapshot :compare-target))
         (lens (plist-get snapshot :active-lens))
         (group (plist-get snapshot :comparison-group))
         (frozen (plist-get snapshot :frozen-context))
         (parts (list (plist-get node :title))))
    (if compare-target
        (setq parts
              (append
               parts
               (list
                (format "compare: %s" (plist-get compare-target :title))
                (format "group: %s" (symbol-name group)))))
      (when lens
        (setq parts
              (append parts
                      (list (format "lens: %s" (symbol-name lens)))))))
    (when (and frozen
               root-node
               (not (equal (plist-get root-node :node_key)
                           (plist-get node :node_key))))
      (setq parts
            (append parts
                    (list (format "root: %s"
                                  (plist-get root-node :title))))))
    (string-join parts "  |  ")))

(defun org-slipbox-buffer--render-comparison-sections (comparison)
  "Render COMPARISON sections for the active comparison group."
  (let* ((left-note (plist-get comparison :left_note))
         (right-note (plist-get comparison :right_note))
         (group (org-slipbox-buffer--current-comparison-group))
         (section-table
          (org-slipbox-buffer--comparison-section-table
           (org-slipbox--plist-sequence (plist-get comparison :sections)))))
    (dolist (kind (org-slipbox-buffer--comparison-section-plan group))
      (when-let ((section (gethash (symbol-name kind) section-table)))
        (org-slipbox-buffer--insert-occurrence-section
         (org-slipbox-buffer--comparison-section-heading section left-note right-note)
         (org-slipbox--plist-sequence (plist-get section :entries))
         (org-slipbox-buffer--comparison-empty-message section)
         #'org-slipbox-buffer--insert-comparison-entry)))))

(defun org-slipbox-buffer--comparison-section-plan (&optional group)
  "Return the comparison section plan for GROUP or the active group."
  (let ((plan (alist-get (or group (org-slipbox-buffer--current-comparison-group))
                         org-slipbox-buffer-comparison-group-plans)))
    (unless plan
      (user-error "No org-slipbox comparison section plan for group %S"
                  (or group (org-slipbox-buffer--current-comparison-group))))
    plan))

(defun org-slipbox-buffer--comparison-section-table (sections)
  "Return a hash table of comparison SECTIONS keyed by kind."
  (let ((table (make-hash-table :test #'equal)))
    (dolist (section sections table)
      (puthash (plist-get section :kind) section table))))

(defun org-slipbox-buffer--comparison-section-heading (section left-note right-note)
  "Return the rendered heading for SECTION between LEFT-NOTE and RIGHT-NOTE."
  (pcase (plist-get section :kind)
    ("shared-refs" "Shared Refs")
    ("shared-planning-dates" "Shared Planning Dates")
    ("left-only-refs"
     (format "Refs only in %s" (plist-get left-note :title)))
    ("right-only-refs"
     (format "Refs only in %s" (plist-get right-note :title)))
    ("shared-backlinks" "Shared Backlinks")
    ("shared-forward-links" "Shared Forward Links")
    ("contrasting-task-states" "Contrasting Task States")
    ("planning-tensions" "Planning Tensions")
    ("indirect-connectors" "Indirect Connectors")
    (_
     (user-error "Unsupported comparison section kind %S"
                 (plist-get section :kind)))))

(defun org-slipbox-buffer--comparison-empty-message (section)
  "Return the empty-message string for comparison SECTION."
  (pcase (plist-get section :kind)
    ("shared-refs" "No shared refs found.")
    ("shared-planning-dates" "No shared planning dates found.")
    ("left-only-refs" "No left-only refs found.")
    ("right-only-refs" "No right-only refs found.")
    ("shared-backlinks" "No shared backlinks found.")
    ("shared-forward-links" "No shared forward links found.")
    ("contrasting-task-states" "No contrasting task states found.")
    ("planning-tensions" "No planning tensions found.")
    ("indirect-connectors" "No indirect connectors found.")
    (_
     (user-error "Unsupported comparison section kind %S"
                 (plist-get section :kind)))))

(defun org-slipbox-buffer--insert-comparison-entry (entry)
  "Insert a comparison ENTRY."
  (pcase (plist-get entry :kind)
    ("reference"
     (insert (plist-get entry :reference))
     (org-slipbox-buffer--insert-explanation entry))
    ("node"
     (org-slipbox-buffer--insert-node-button
      (plist-get entry :node)
      "Pivot within comparison")
     (org-slipbox-buffer--insert-explanation entry))
    ("planning-relation"
     (insert (plist-get entry :date))
     (org-slipbox-buffer--insert-explanation entry))
    ("task-state"
     (insert (format "%s <> %s"
                     (plist-get entry :left_todo_keyword)
                     (plist-get entry :right_todo_keyword)))
     (org-slipbox-buffer--insert-explanation entry))
    (_
     (user-error "Unsupported comparison entry kind %S" (plist-get entry :kind)))))

(cl-defun org-slipbox-buffer-node-section (node &key heading)
  "Insert the current NODE summary section.
HEADING overrides the default section title, which is the node title."
  (let ((heading (or heading (plist-get node :title))))
    (org-slipbox-buffer--insert-heading heading)
    (org-slipbox-buffer--insert-metadata-line "File" (plist-get node :file_path))
    (when-let ((mtime (plist-get node :file_mtime_ns)))
      (org-slipbox-buffer--insert-metadata-line
       "Modified"
       (org-slipbox-buffer--format-file-mtime mtime)))
    (when-let ((outline (plist-get node :outline_path)))
      (unless (string-empty-p outline)
        (org-slipbox-buffer--insert-metadata-line "Outline" outline)))
    (when-let ((explicit-id (plist-get node :explicit_id)))
      (org-slipbox-buffer--insert-metadata-line "ID" explicit-id))
    (when-let ((backlink-count (plist-get node :backlink_count)))
      (org-slipbox-buffer--insert-metadata-line
       "Backlinks"
       (number-to-string backlink-count)))
    (when-let ((forward-link-count (plist-get node :forward_link_count)))
      (org-slipbox-buffer--insert-metadata-line
       "Forward Links"
       (number-to-string forward-link-count)))
    (when-let ((aliases (org-slipbox--plist-sequence (plist-get node :aliases))))
      (when aliases
        (org-slipbox-buffer--insert-metadata-line "Aliases" (string-join aliases ", "))))
    (when-let ((tags (org-slipbox--plist-sequence (plist-get node :tags))))
      (when tags
        (org-slipbox-buffer--insert-metadata-line "Tags" (string-join tags ", "))))
    (insert "\n")
    t))

(cl-defun org-slipbox-buffer-refs-section (node &key (section-heading "Refs"))
  "Insert the ref section for NODE using SECTION-HEADING."
  (let ((refs (org-slipbox--plist-sequence (plist-get node :refs))))
    (when refs
      (insert section-heading "\n")
      (insert (make-string (length section-heading) ?-) "\n")
      (dolist (reference refs)
        (insert reference "\n"))
      (insert "\n")
      t)))

(cl-defun org-slipbox-buffer-backlinks-section
    (node &key (unique (org-slipbox-buffer--current-structure-unique)) show-backlink-p
          (section-heading "Backlinks")
          (limit (org-slipbox-buffer--current-query-limit)))
  "Insert a backlink section for NODE.
When UNIQUE is non-nil, only show the first backlink occurrence per
source node. SHOW-BACKLINK-P filters backlink entries when non-nil.
SECTION-HEADING overrides the rendered heading. LIMIT bounds the query."
  (let* ((backlinks (org-slipbox-buffer--backlinks node unique limit))
         (backlinks (if show-backlink-p
                        (seq-filter show-backlink-p backlinks)
                      backlinks)))
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     backlinks
     "No backlinks found."
     #'org-slipbox-buffer--insert-backlink-entry)))

(cl-defun org-slipbox-buffer-forward-links-section
    (node &key (unique (org-slipbox-buffer--current-structure-unique)) show-forward-link-p
          (section-heading "Forward Links")
          (limit (org-slipbox-buffer--current-query-limit)))
  "Insert a forward-links section for NODE.
When UNIQUE is non-nil, only show the first forward-link occurrence per
destination node. SHOW-FORWARD-LINK-P filters forward-link entries when
non-nil. SECTION-HEADING overrides the rendered heading. LIMIT bounds the
query."
  (let* ((forward-links (org-slipbox-buffer--forward-links node unique limit))
         (forward-links (if show-forward-link-p
                            (seq-filter show-forward-link-p forward-links)
                          forward-links)))
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     forward-links
     "No forward links found."
     (lambda (entry)
       (org-slipbox-buffer--insert-forward-link-entry node entry)))))

(cl-defun org-slipbox-buffer-reflinks-section (node &key (section-heading "Reflinks"))
  "Insert a reflink section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     (org-slipbox-buffer--reflinks node)
     "No reflinks found."
     #'org-slipbox-buffer--insert-reflink-entry)))

(cl-defun org-slipbox-buffer-unlinked-references-section
    (node &key (section-heading "Unlinked References"))
  "Insert an unlinked-reference section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     (org-slipbox-buffer--unlinked-references node)
     "No unlinked references found."
     #'org-slipbox-buffer--insert-unlinked-reference-entry)))

(cl-defun org-slipbox-buffer-time-neighbors-section
    (node &key (section-heading "Time Neighbors"))
  "Insert a time-neighbor section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--time-neighbors node)
   "No time neighbors found."
   #'org-slipbox-buffer--insert-anchor-entry))

(cl-defun org-slipbox-buffer-task-neighbors-section
    (node &key (section-heading "Task Neighbors"))
  "Insert a task-neighbor section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--task-neighbors node)
   "No task neighbors found."
   #'org-slipbox-buffer--insert-anchor-entry))

(cl-defun org-slipbox-buffer-bridge-candidates-section
    (node &key (section-heading "Bridge Candidates"))
  "Insert a bridge-candidate section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--bridge-candidates node)
   "No bridge candidates found."
   #'org-slipbox-buffer--insert-anchor-entry))

(cl-defun org-slipbox-buffer-dormant-notes-section
    (node &key (section-heading "Dormant Notes"))
  "Insert a dormant-note section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--dormant-notes node)
   "No dormant notes found."
   #'org-slipbox-buffer--insert-anchor-entry))

(cl-defun org-slipbox-buffer-unresolved-tasks-section
    (node &key (section-heading "Unresolved Tasks"))
  "Insert an unresolved-task section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--unresolved-tasks node)
   "No unresolved tasks found."
   #'org-slipbox-buffer--insert-anchor-entry))

(cl-defun org-slipbox-buffer-weakly-integrated-notes-section
    (node &key (section-heading "Weakly Integrated Notes"))
  "Insert a weakly integrated note section for NODE using SECTION-HEADING."
  (org-slipbox-buffer--insert-occurrence-section
   section-heading
   (org-slipbox-buffer--weakly-integrated-notes node)
   "No weakly integrated notes found."
   #'org-slipbox-buffer--insert-anchor-entry))

(defun org-slipbox-buffer--insert-heading (text)
  "Insert section heading TEXT."
  (insert text "\n")
  (insert (make-string (length text) ?=) "\n\n"))

(defun org-slipbox-buffer--insert-metadata-line (label value)
  "Insert LABEL and VALUE on one line."
  (insert (propertize (format "%-7s " (concat label ":")) 'face 'bold)
          value
          "\n"))

(defun org-slipbox-buffer--format-file-mtime (mtime-ns)
  "Return a display string for MTIME-NS."
  (format-time-string
   "%Y-%m-%d"
   (seconds-to-time (/ (float mtime-ns) 1000000000.0))))

(defun org-slipbox-buffer--insert-occurrence-section (title entries empty-message inserter)
  "Insert section TITLE using ENTRIES or EMPTY-MESSAGE via INSERTER."
  (insert title "\n")
  (insert (make-string (length title) ?-) "\n")
  (if entries
      (dolist (entry entries)
        (funcall inserter entry)
        (insert "\n"))
    (insert empty-message "\n"))
  (insert "\n")
  t)

(defun org-slipbox-buffer--insert-node-button (node &optional help-echo)
  "Insert a button for NODE with optional HELP-ECHO."
  (insert-text-button
   (org-slipbox--node-display node)
   'follow-link t
   'help-echo (or help-echo "Pivot or visit node")
   'action (lambda (_button)
             (org-slipbox-buffer--activate-node node))))

(defun org-slipbox-buffer--insert-anchor-button (anchor &optional help-echo)
  "Insert a button for ANCHOR with optional HELP-ECHO."
  (insert-text-button
   (org-slipbox--node-display anchor)
   'follow-link t
   'help-echo (or help-echo "Pivot or visit related anchor")
   'action (lambda (_button)
             (org-slipbox-buffer--activate-anchor anchor))))

(defun org-slipbox-buffer--insert-location-button (file row col help-echo)
  "Insert a location button for FILE at ROW and COL with HELP-ECHO."
  (insert-text-button
   (format "%s:%s:%s" file row col)
   'follow-link t
   'face 'shadow
   'help-echo help-echo
   'action (lambda (_button)
             (org-slipbox-buffer--visit-location file row col))))

(defun org-slipbox-buffer--note-p (node)
  "Return non-nil when NODE already denotes a canonical note."
  (let ((kind (plist-get node :kind)))
    (or (equal kind "file")
        (eq kind 'file)
        (plist-get node :explicit_id))))

(defun org-slipbox-buffer--resolve-anchor-pivot-node (anchor)
  "Resolve a canonical pivot node for ANCHOR."
  (if (org-slipbox-buffer--note-p anchor)
      anchor
    (org-slipbox-rpc-node-at-point
     (expand-file-name (plist-get anchor :file_path) org-slipbox-directory)
     (plist-get anchor :line))))

(defun org-slipbox-buffer--activate-node (node)
  "Activate NODE from the current org-slipbox buffer."
  (if (org-slipbox-buffer--dedicated-p)
      (org-slipbox-buffer--pivot-to-node node)
    (org-slipbox--visit-node node)))

(defun org-slipbox-buffer--activate-anchor (anchor)
  "Activate ANCHOR from the current org-slipbox buffer."
  (if-let ((node (org-slipbox-buffer--resolve-anchor-pivot-node anchor)))
      (org-slipbox-buffer--activate-node node)
    (org-slipbox-buffer--visit-location
     (plist-get anchor :file_path)
     (plist-get anchor :line)
     1)))

(defun org-slipbox-buffer--pivot-to-node (node)
  "Pivot the dedicated buffer to NODE."
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session))
         (root-node (if (org-slipbox-buffer-session-frozen-context session)
                        (org-slipbox-buffer-session-root-node session)
                      node)))
    (setq snapshot (plist-put snapshot :current-node node))
    (setq snapshot (plist-put snapshot :current-focus-key
                              (plist-get node :node_key)))
    (setq snapshot (plist-put snapshot :root-node root-node))
    (unless (org-slipbox-buffer-session-frozen-context session)
      (setq snapshot (plist-put snapshot :root-focus-key
                                (plist-get root-node :node_key))))
    (org-slipbox-buffer--transition-dedicated snapshot)))

(defun org-slipbox-buffer--format-explanation-list (values)
  "Format VALUES as a comma-separated explanation list."
  (mapconcat #'identity values ", "))

(defun org-slipbox-buffer--shared-reference-summary (explanation)
  "Return a shared-reference summary string for EXPLANATION."
  (let* ((references (plist-get explanation :references))
         (label (if (= (length references) 1) "shared ref" "shared refs")))
    (format "%s: %s"
            label
            (org-slipbox-buffer--format-explanation-list references))))

(defun org-slipbox-buffer--planning-field-label (field)
  "Return a short display label for planning FIELD."
  (pcase field
    ("scheduled" "scheduled")
    ("deadline" "deadline")
    (_ "unknown")))

(defun org-slipbox-buffer--planning-relation-summary (relation)
  "Return a concise display string for planning RELATION."
  (format "%s->%s %s"
          (org-slipbox-buffer--planning-field-label
           (plist-get relation :source_field))
          (org-slipbox-buffer--planning-field-label
           (plist-get relation :candidate_field))
          (plist-get relation :date)))

(defun org-slipbox-buffer--planning-relations-summary (relations)
  "Return a display string for planning RELATIONS."
  (org-slipbox-buffer--format-explanation-list
   (mapcar #'org-slipbox-buffer--planning-relation-summary
           (append relations nil))))

(defun org-slipbox-buffer--bridge-via-note-summary (explanation)
  "Return a title summary for bridge-note evidence in EXPLANATION."
  (let ((counts (make-hash-table :test #'equal))
        ordered-titles)
    (dolist (note (append (plist-get explanation :via_notes) nil))
      (let ((title (plist-get note :title)))
        (unless (gethash title counts)
          (setq ordered-titles (append ordered-titles (list title))))
        (puthash title (1+ (gethash title counts 0)) counts)))
    (org-slipbox-buffer--format-explanation-list
     (mapcar (lambda (title)
               (let ((count (gethash title counts)))
                 (if (> count 1)
                     (format "%s (%s)" title count)
                   title)))
             ordered-titles))))

(defun org-slipbox-buffer--inline-explanation-string (entry)
  "Return an inline explanation string for ENTRY."
  (when-let ((explanation (plist-get entry :explanation)))
    (pcase (plist-get explanation :kind)
      ("backlink" "direct backlink")
      ("forward-link" "direct forward link")
      ("shared-reference"
       (format "shared ref: %s" (plist-get explanation :reference)))
      ("left-only-reference" "only in current note")
      ("right-only-reference" "only in compare target")
      ("shared-backlink" "shared backlink")
      ("shared-forward-link" "shared forward link")
      ("indirect-connector"
       (pcase (plist-get explanation :direction)
         ("left-to-right" "current note -> compare target")
         ("right-to-left" "compare target -> current note")
         ("bidirectional" "bidirectional connector")))
      ("unlinked-reference"
       (format "unlinked mention: %s" (plist-get explanation :matched_text))))))

(defun org-slipbox-buffer--block-explanation-lines (entry)
  "Return block-style explanation lines for ENTRY."
  (when-let ((explanation (plist-get entry :explanation)))
    (pcase (plist-get explanation :kind)
      ("shared-planning-date"
       (list
        (format "because planning overlap: current note %s, compare target %s"
                (org-slipbox-buffer--planning-field-label
                 (plist-get entry :left_field))
                (org-slipbox-buffer--planning-field-label
                 (plist-get entry :right_field)))))
      ("contrasting-task-state"
       (list
        (format "because task tension: %s <> %s"
                (plist-get entry :left_todo_keyword)
                (plist-get entry :right_todo_keyword))))
      ("planning-tension"
       (list
        (format "because planning tension: current note %s, compare target %s"
                (org-slipbox-buffer--planning-field-label
                 (plist-get entry :left_field))
                (org-slipbox-buffer--planning-field-label
                 (plist-get entry :right_field)))))
      ("bridge-candidate"
       (list
        (format "because %s"
                (org-slipbox-buffer--shared-reference-summary explanation))
        (format "via bridge notes: %s"
                (org-slipbox-buffer--bridge-via-note-summary explanation))))
      ("dormant-shared-reference"
       (list
        (format "because %s"
                (org-slipbox-buffer--shared-reference-summary explanation))
        "state: older untouched material"))
      ("unresolved-shared-reference"
       (list
        (format "because %s"
                (org-slipbox-buffer--shared-reference-summary explanation))
        (format "task state: %s"
                (plist-get explanation :todo_keyword))))
      ("weakly-integrated-shared-reference"
       (list
        (format "because %s"
                (org-slipbox-buffer--shared-reference-summary explanation))
        (format "structural links: %s"
                (plist-get explanation :structural_link_count))))
      ("time-neighbor"
       (list
        (format "because planning overlap: %s"
                (org-slipbox-buffer--planning-relations-summary
                 (plist-get explanation :relations)))))
      ("task-neighbor"
       (let ((todo-keyword (plist-get explanation :shared_todo_keyword))
             (planning-relations (append (plist-get explanation :planning_relations) nil))
             lines)
         (when todo-keyword
           (push (format "because shared task state: %s" todo-keyword) lines))
         (when planning-relations
           (push (format "%splanning overlap: %s"
                         (if lines "" "because ")
                         (org-slipbox-buffer--planning-relations-summary
                          planning-relations))
                 lines))
         (nreverse lines))))))

(defun org-slipbox-buffer--insert-explanation (entry)
  "Insert ENTRY's explanation payload when it is present."
  (cond
   ((when-let ((lines (org-slipbox-buffer--block-explanation-lines entry)))
      (dolist (line lines)
        (insert "\n  " (propertize line 'face 'italic)))
      t))
   ((when-let ((reason (org-slipbox-buffer--inline-explanation-string entry)))
      (insert " " (propertize reason 'face 'italic))
      t))))

(defun org-slipbox-buffer--insert-backlink-entry (entry)
  "Insert a preview-rich backlink ENTRY."
  (let* ((source-node (plist-get entry :source_note))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview)))
    (org-slipbox-buffer--insert-node-button source-node "Pivot to backlink source note")
    (insert " ")
    (org-slipbox-buffer--insert-location-button
     file row col "Visit backlink occurrence")
    (org-slipbox-buffer--insert-explanation entry)
    (insert
            "\n  "
            preview)))

(defun org-slipbox-buffer--insert-forward-link-entry (node entry)
  "Insert a preview-rich forward-link ENTRY for source NODE."
  (let* ((destination-node (plist-get entry :destination_note))
         (file (plist-get node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview)))
    (org-slipbox-buffer--insert-node-button destination-node "Pivot to linked note")
    (insert " ")
    (org-slipbox-buffer--insert-location-button
     file row col "Visit forward-link occurrence")
    (org-slipbox-buffer--insert-explanation entry)
    (insert
            "\n  "
            preview)))

(defun org-slipbox-buffer--insert-reflink-entry (entry)
  "Insert a preview-rich reflink ENTRY."
  (let* ((source-node (plist-get entry :source_anchor))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview))
         (matched-reference (plist-get entry :matched_reference)))
    (org-slipbox-buffer--insert-anchor-button source-node "Pivot to reflink source note")
    (insert " ")
    (org-slipbox-buffer--insert-location-button
     file row col "Visit reflink occurrence")
    (org-slipbox-buffer--insert-explanation entry)
    (when (and (null (plist-get entry :explanation)) matched-reference)
      (insert " " (propertize matched-reference 'face 'italic)))
    (insert "\n  " preview)))

(defun org-slipbox-buffer--insert-unlinked-reference-entry (entry)
  "Insert a preview-rich unlinked-reference ENTRY."
  (let* ((source-node (plist-get entry :source_anchor))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview))
         (matched-text (plist-get entry :matched_text)))
    (org-slipbox-buffer--insert-anchor-button
     source-node
     "Pivot to unlinked-reference source note")
    (insert " ")
    (org-slipbox-buffer--insert-location-button
     file row col "Visit unlinked-reference occurrence")
    (org-slipbox-buffer--insert-explanation entry)
    (when (and (null (plist-get entry :explanation)) matched-text)
      (insert " " (propertize matched-text 'face 'italic)))
    (insert "\n  " preview)))

(defun org-slipbox-buffer--insert-anchor-entry (entry)
  "Insert an anchor-backed exploration ENTRY."
  (let* ((anchor (plist-get entry :anchor))
         (file (plist-get anchor :file_path))
         (row (plist-get anchor :line))
         (col 1))
    (org-slipbox-buffer--insert-anchor-button anchor "Pivot to related note")
    (insert " ")
    (org-slipbox-buffer--insert-location-button
     file row col "Visit related anchor")
    (org-slipbox-buffer--insert-explanation entry)))

(defun org-slipbox-buffer--visit-location (file row col)
  "Visit FILE at ROW and COL."
  (find-file (if (file-name-absolute-p file)
                 file
               (expand-file-name file org-slipbox-directory)))
  (goto-char (point-min))
  (forward-line (1- row))
  (forward-char (1- col))
  (when (and (fboundp 'org-fold-show-context)
             (org-invisible-p))
    (org-fold-show-context)))

(provide 'org-slipbox-buffer-render)

;;; org-slipbox-buffer-render.el ends here
