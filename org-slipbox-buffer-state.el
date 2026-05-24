;;; org-slipbox-buffer-state.el --- State helpers for org-slipbox context buffers -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.14.0
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

;; State helpers for org-slipbox context buffers.

;;; Code:

(require 'cl-lib)
(require 'org-slipbox-discovery)

(defvar org-slipbox-buffer "*org-slipbox*"
  "Name of the persistent org-slipbox context buffer.")

(defcustom org-slipbox-buffer-expensive-sections 'dedicated
  "When expensive discovery sections should be rendered.
`dedicated' renders them only in dedicated org-slipbox buffers,
`always' renders them everywhere, and nil disables them."
  :type '(choice
          (const :tag "Never" nil)
          (const :tag "Dedicated Buffers" dedicated)
          (const :tag "Always" always))
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-persistent-sections
  (list #'org-slipbox-buffer-node-section
        #'org-slipbox-buffer-refs-section
        #'org-slipbox-buffer-backlinks-section
        #'org-slipbox-buffer-forward-links-section)
  "Cheap section plan rendered by persistent org-slipbox buffers.

Each item is either a function called with the current node, or a
list whose car is a function and whose remaining items are passed as
additional arguments. For example:

  (org-slipbox-buffer-backlinks-section :unique t
                                        :section-heading \"Unique Backlinks\")"
  :type `(repeat (choice (symbol :tag "Function")
                         (list :tag "Function with arguments"
                               (symbol :tag "Function")
                               (repeat :tag "Arguments" :inline t (sexp :tag "Arg")))))
  :group 'org-slipbox)

(defconst org-slipbox-buffer-lenses
  '(structure refs time tasks bridges dormant unresolved)
  "Declared exploration lenses supported by the dedicated buffer.")

(defcustom org-slipbox-buffer-lens-plans
  '((structure
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-backlinks-section
     org-slipbox-buffer-forward-links-section)
    (refs
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-reflinks-section
     org-slipbox-buffer-unlinked-references-section)
    (time
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-time-neighbors-section)
    (tasks
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-task-neighbors-section)
    (bridges
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-bridge-candidates-section)
    (dormant
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-dormant-notes-section)
    (unresolved
     org-slipbox-buffer-node-section
     org-slipbox-buffer-refs-section
     org-slipbox-buffer-unresolved-tasks-section
     org-slipbox-buffer-weakly-integrated-notes-section))
  "Dedicated-buffer section plans keyed by exploration lens.

Each plan is an alist entry whose car is a declared lens symbol and
whose cdr is a list of section specifications rendered in order."
  :type `(alist
          :key-type (choice ,@(mapcar (lambda (lens)
                                        `(const :tag ,(symbol-name lens) ,lens))
                                      org-slipbox-buffer-lenses))
          :value-type (repeat (choice (symbol :tag "Function")
                                      (list :tag "Function with arguments"
                                            (symbol :tag "Function")
                                            (repeat :tag "Arguments"
                                                    :inline t
                                                    (sexp :tag "Arg"))))))
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-postrender-functions nil
  "Functions run after an org-slipbox buffer has been rendered.
Each function is called with the rendered buffer as current."
  :type 'hook
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-section-filter-function nil
  "Optional predicate controlling whether a section should render.
When non-nil, this function is called with SECTION-SPEC and NODE.
Return non-nil to render the section, or nil to skip it."
  :type '(choice (const :tag "None" nil) function)
  :group 'org-slipbox)

(defconst org-slipbox-buffer-comparison-groups '(all overlap divergence tension)
  "Declared comparison groups supported by the dedicated buffer.")

(defcustom org-slipbox-buffer-comparison-group-plans
  '((all
     shared-refs
     shared-planning-dates
     left-only-refs
     right-only-refs
     shared-backlinks
     shared-forward-links
     contrasting-task-states
     planning-tensions
     indirect-connectors)
    (overlap
     shared-refs
     shared-planning-dates
     shared-backlinks
     shared-forward-links)
    (divergence
     left-only-refs
     right-only-refs)
    (tension
     contrasting-task-states
     planning-tensions
     indirect-connectors))
  "Comparison section plans keyed by dedicated comparison group."
  :type `(alist
          :key-type (choice ,@(mapcar (lambda (group)
                                        `(const :tag ,(symbol-name group) ,group))
                                      org-slipbox-buffer-comparison-groups))
          :value-type (repeat symbol))
  :group 'org-slipbox)

(defconst org-slipbox-buffer-default-query-limit 200
  "Default per-section query limit for dedicated exploration and comparison.")

(cl-defstruct org-slipbox-buffer-session
  "Explicit session state for an org-slipbox context buffer."
  kind
  current-node
  root-node
  current-focus-key
  root-focus-key
  active-lens
  compare-target
  comparison-group
  query-limit
  structure-unique
  trail
  trail-index
  history
  future
  frozen-context
  lens-cache
  comparison-cache)

(defvar-local org-slipbox-buffer-session nil
  "Explicit session state for the current org-slipbox context buffer.")

(put 'org-slipbox-buffer-session 'permanent-local t)

(defun org-slipbox-buffer--current-section-plan ()
  "Return the active section plan for the current buffer."
  (if (org-slipbox-buffer--dedicated-p)
      (org-slipbox-buffer--dedicated-section-plan)
    org-slipbox-buffer-persistent-sections))

(defun org-slipbox-buffer--dedicated-section-plan (&optional lens)
  "Return the dedicated section plan for LENS or the current lens."
  (let ((plan (alist-get (or lens (org-slipbox-buffer--current-lens))
                         org-slipbox-buffer-lens-plans)))
    (unless plan
      (user-error "No org-slipbox dedicated section plan for lens %S"
                  (or lens (org-slipbox-buffer--current-lens))))
    plan))

(defun org-slipbox-buffer--make-persistent-session (&optional node)
  "Return a persistent context-buffer session for NODE."
  (make-org-slipbox-buffer-session
   :kind 'persistent
   :current-node node
   :root-node node
   :current-focus-key (plist-get node :node_key)
   :root-focus-key (plist-get node :node_key)))

(defun org-slipbox-buffer--normalize-persistent-session (session node)
  "Normalize persistent SESSION around NODE.
Dedicated-only state must not survive on the persistent tracking path."
  (setf (org-slipbox-buffer-session-kind session) 'persistent
        (org-slipbox-buffer-session-current-node session) node
        (org-slipbox-buffer-session-root-node session) node
        (org-slipbox-buffer-session-current-focus-key session)
        (plist-get node :node_key)
        (org-slipbox-buffer-session-root-focus-key session)
        (plist-get node :node_key)
        (org-slipbox-buffer-session-active-lens session) nil
        (org-slipbox-buffer-session-compare-target session) nil
        (org-slipbox-buffer-session-comparison-group session) nil
        (org-slipbox-buffer-session-query-limit session) nil
        (org-slipbox-buffer-session-structure-unique session) nil
        (org-slipbox-buffer-session-trail session) nil
        (org-slipbox-buffer-session-trail-index session) nil
        (org-slipbox-buffer-session-history session) nil
        (org-slipbox-buffer-session-future session) nil
        (org-slipbox-buffer-session-frozen-context session) nil
        (org-slipbox-buffer-session-comparison-cache session) nil)
  session)

(defun org-slipbox-buffer--make-dedicated-session (node)
  "Return a dedicated context-buffer session rooted at NODE."
  (make-org-slipbox-buffer-session
   :kind 'dedicated
   :current-node node
   :root-node node
   :current-focus-key (plist-get node :node_key)
   :root-focus-key (plist-get node :node_key)
   :active-lens 'structure
   :comparison-group 'all
   :query-limit org-slipbox-buffer-default-query-limit
   :structure-unique nil
   :frozen-context t))

(defun org-slipbox-buffer--session-node (&optional session)
  "Return the current node for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-current-node session)))

(defun org-slipbox-buffer--clear-lens-cache (&optional session)
  "Clear cached exploration results for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (setf (org-slipbox-buffer-session-lens-cache session) nil)))

(defun org-slipbox-buffer--clear-comparison-cache (&optional session)
  "Clear cached comparison results for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (setf (org-slipbox-buffer-session-comparison-cache session) nil)))

(defun org-slipbox-buffer--clear-session-caches (&optional session)
  "Clear transient query caches for SESSION or the current buffer."
  (org-slipbox-buffer--clear-lens-cache session)
  (org-slipbox-buffer--clear-comparison-cache session))

(defun org-slipbox-buffer--section-function (section)
  "Return the function designator for SECTION."
  (pcase section
    ((pred functionp) section)
    (`(,fn . ,_) fn)
    (_ nil)))

(defun org-slipbox-buffer--current-lens (&optional session)
  "Return the active exploration lens for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-active-lens session)))

(defun org-slipbox-buffer--current-focus-key (&optional session)
  "Return the active exploration focus key for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (or (org-slipbox-buffer-session-current-focus-key session)
        (plist-get (org-slipbox-buffer-session-current-node session) :node_key))))

(defun org-slipbox-buffer--root-focus-key (&optional session)
  "Return the root exploration focus key for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (or (org-slipbox-buffer-session-root-focus-key session)
        (plist-get (org-slipbox-buffer-session-root-node session) :node_key)
        (org-slipbox-buffer--current-focus-key session))))

(defun org-slipbox-buffer--current-query-limit (&optional session)
  "Return the active query limit for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (or (org-slipbox-buffer-session-query-limit session)
        org-slipbox-buffer-default-query-limit)))

(defun org-slipbox-buffer--current-structure-unique (&optional session)
  "Return the active structure-unique flag for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (and (org-slipbox-buffer-session-structure-unique session) t)))

(defun org-slipbox-buffer--compare-target (&optional session)
  "Return the comparison target for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-compare-target session)))

(defun org-slipbox-buffer--current-comparison-group (&optional session)
  "Return the active comparison group for SESSION or the current buffer."
  (or (when-let ((session (or session org-slipbox-buffer-session)))
        (org-slipbox-buffer-session-comparison-group session))
      'all))

(defun org-slipbox-buffer--comparison-active-p (&optional session)
  "Return non-nil when SESSION or the current buffer is in comparison mode."
  (not (null (org-slipbox-buffer--compare-target session))))

(defun org-slipbox-buffer--trail (&optional session)
  "Return the explicit trail for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-trail session)))

(defun org-slipbox-buffer--trail-position (&optional session)
  "Return the active trail position for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-trail-index session)))

(defun org-slipbox-buffer--trail-active-p (&optional session)
  "Return non-nil when SESSION or the current buffer has an explicit trail."
  (not (null (org-slipbox-buffer--trail session))))

(defun org-slipbox-buffer--trail-detached-p (&optional session)
  "Return non-nil when SESSION has an active trail but current state is detached."
  (let ((session (or session org-slipbox-buffer-session)))
    (and session
         (org-slipbox-buffer--trail-active-p session)
         (not (org-slipbox-buffer--trail-attached-p session)))))

(defun org-slipbox-buffer--trail-attached-p (&optional session)
  "Return non-nil when SESSION or the current buffer is on its trail cursor."
  (let* ((session (or session org-slipbox-buffer-session))
         (trail (and session (org-slipbox-buffer-session-trail session)))
         (trail-index (and session (org-slipbox-buffer-session-trail-index session))))
    (and trail
         trail-index
         (equal (nth trail-index trail)
                (org-slipbox-buffer--history-snapshot session)))))

(defun org-slipbox-buffer--trail-snapshot-index (snapshot &optional session)
  "Return the trail index of SNAPSHOT for SESSION, or nil when absent."
  (cl-position snapshot
               (org-slipbox-buffer--trail session)
               :test #'equal))

(defun org-slipbox-buffer--history-snapshot (&optional session)
  "Return a navigation snapshot for SESSION or the current buffer."
  (let ((session (or session org-slipbox-buffer-session)))
    (list :current-node (org-slipbox-buffer-session-current-node session)
          :root-node (org-slipbox-buffer-session-root-node session)
          :current-focus-key (org-slipbox-buffer--current-focus-key session)
          :root-focus-key (org-slipbox-buffer--root-focus-key session)
          :active-lens (org-slipbox-buffer-session-active-lens session)
          :compare-target (org-slipbox-buffer-session-compare-target session)
          :comparison-group (org-slipbox-buffer--current-comparison-group session)
          :query-limit (org-slipbox-buffer--current-query-limit session)
          :structure-unique (org-slipbox-buffer--current-structure-unique session)
          :frozen-context (org-slipbox-buffer-session-frozen-context session))))

(defun org-slipbox-buffer--apply-history-snapshot (session snapshot)
  "Apply SNAPSHOT to SESSION and clear its transient caches."
  (setf (org-slipbox-buffer-session-current-node session)
        (plist-get snapshot :current-node)
        (org-slipbox-buffer-session-root-node session)
        (plist-get snapshot :root-node)
        (org-slipbox-buffer-session-current-focus-key session)
        (or (plist-get snapshot :current-focus-key)
            (plist-get (plist-get snapshot :current-node) :node_key))
        (org-slipbox-buffer-session-root-focus-key session)
        (or (plist-get snapshot :root-focus-key)
            (plist-get (plist-get snapshot :root-node) :node_key)
            (plist-get snapshot :current-focus-key)
            (plist-get (plist-get snapshot :current-node) :node_key))
        (org-slipbox-buffer-session-active-lens session)
        (plist-get snapshot :active-lens)
        (org-slipbox-buffer-session-compare-target session)
        (plist-get snapshot :compare-target)
        (org-slipbox-buffer-session-comparison-group session)
        (plist-get snapshot :comparison-group)
        (org-slipbox-buffer-session-query-limit session)
        (or (plist-get snapshot :query-limit)
            org-slipbox-buffer-default-query-limit)
        (org-slipbox-buffer-session-structure-unique session)
        (and (plist-get snapshot :structure-unique) t)
        (org-slipbox-buffer-session-frozen-context session)
        (plist-get snapshot :frozen-context)
        (org-slipbox-buffer-session-lens-cache session) nil
        (org-slipbox-buffer-session-comparison-cache session) nil))

(defun org-slipbox-buffer--reconcile-trail-position (session)
  "Align SESSION's trail cursor when its current state already exists on the trail."
  (when-let ((index (org-slipbox-buffer--trail-snapshot-index
                     (org-slipbox-buffer--history-snapshot session)
                     session)))
    (setf (org-slipbox-buffer-session-trail-index session) index)))

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

(provide 'org-slipbox-buffer-state)

;;; org-slipbox-buffer-state.el ends here
