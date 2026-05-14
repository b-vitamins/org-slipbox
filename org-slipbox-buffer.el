;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.13.2
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

;; Context buffer commands for `org-slipbox'.

;;; Code:

(require 'button)
(require 'cl-lib)
(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-files)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

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

(defvar org-slipbox-buffer-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "l") #'org-slipbox-buffer-switch-lens)
    (define-key map (kbd "o") #'org-slipbox-buffer-load-artifact)
    (define-key map (kbd "s") #'org-slipbox-buffer-save-artifact)
    (define-key map (kbd "c") #'org-slipbox-buffer-set-compare-target)
    (define-key map (kbd "C") #'org-slipbox-buffer-clear-compare-target)
    (define-key map (kbd "g") #'org-slipbox-buffer-switch-comparison-group)
    (define-key map (kbd "a") #'org-slipbox-buffer-trail-add)
    (define-key map (kbd "{") #'org-slipbox-buffer-trail-back)
    (define-key map (kbd "}") #'org-slipbox-buffer-trail-forward)
    (define-key map (kbd "T") #'org-slipbox-buffer-trail-clear)
    (define-key map (kbd "[") #'org-slipbox-buffer-history-back)
    (define-key map (kbd "]") #'org-slipbox-buffer-history-forward)
    (define-key map (kbd "f") #'org-slipbox-buffer-toggle-frozen-context)
    map)
  "Keymap for `org-slipbox-buffer-mode'.")

(define-derived-mode org-slipbox-buffer-mode special-mode "org-slipbox"
  "Major mode for org-slipbox context buffers.")

(define-minor-mode org-slipbox-buffer-persistent-mode
  "Keep the persistent org-slipbox context buffer synchronized with point."
  :global t
  :group 'org-slipbox
  (if org-slipbox-buffer-persistent-mode
      (add-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)
    (remove-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)))

;;;###autoload
(defun org-slipbox-buffer-refresh ()
  "Refresh the current org-slipbox context buffer."
  (interactive)
  (unless (derived-mode-p 'org-slipbox-buffer-mode)
    (user-error "Not in an org-slipbox buffer"))
  (unless (org-slipbox-buffer--session-node)
    (user-error "No org-slipbox node to refresh"))
  (org-slipbox-buffer--clear-session-caches)
  (org-slipbox-buffer-render-contents))

;;;###autoload
(defun org-slipbox-buffer-display-dedicated (node)
  "Display a dedicated org-slipbox buffer for NODE."
  (interactive (list (org-slipbox-buffer--read-node-for-display)))
  (let ((buffer (get-buffer-create (org-slipbox-buffer--dedicated-name node))))
    (with-current-buffer buffer
      (setq-local org-slipbox-buffer-session
                  (org-slipbox-buffer--make-dedicated-session node))
      (org-slipbox-buffer-render-contents))
    (display-buffer buffer)))

;;;###autoload
(defun org-slipbox-buffer-toggle ()
  "Toggle display of the persistent org-slipbox context buffer."
  (interactive)
  (if (get-buffer-window org-slipbox-buffer 'visible)
      (progn
        (quit-window nil (get-buffer-window org-slipbox-buffer))
        (org-slipbox-buffer-persistent-mode -1))
    (display-buffer (get-buffer-create org-slipbox-buffer))
    (org-slipbox-buffer-persistent-redisplay)
    (org-slipbox-buffer-persistent-mode 1)))

(defun org-slipbox-buffer-persistent-redisplay ()
  "Refresh the persistent org-slipbox context buffer from point."
  (when-let ((node (org-slipbox-node-at-point)))
    (with-current-buffer (get-buffer-create org-slipbox-buffer)
      (let* ((session (or org-slipbox-buffer-session
                          (org-slipbox-buffer--make-persistent-session)))
             (node-changed
              (not (equal node (org-slipbox-buffer-session-current-node session)))))
        (org-slipbox-buffer--normalize-persistent-session session node)
        (when node-changed
          (setf (org-slipbox-buffer-session-lens-cache session) nil)
          (setq-local org-slipbox-buffer-session session)
          (org-slipbox-buffer-render-contents)
          (add-hook 'kill-buffer-hook #'org-slipbox-buffer--persistent-cleanup-h nil t))))))

(defun org-slipbox-buffer-switch-lens (lens)
  "Switch the dedicated buffer to exploration LENS."
  (interactive
   (list
    (intern
     (completing-read
      "Lens: "
      (mapcar #'symbol-name org-slipbox-buffer-lenses)
      nil
      t
      nil
      nil
      (and (org-slipbox-buffer--current-lens)
           (symbol-name (org-slipbox-buffer--current-lens)))))))
  (when (org-slipbox-buffer--comparison-active-p)
    (user-error "Exit comparison mode before switching lenses"))
  (unless (memq lens org-slipbox-buffer-lenses)
    (user-error "Unsupported org-slipbox lens %S" lens))
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session)))
    (setq snapshot (plist-put snapshot :active-lens lens))
    (unless (memq lens '(refs time tasks))
      (setq snapshot
            (plist-put snapshot :current-focus-key
                       (plist-get (plist-get snapshot :current-node) :node_key)))
      (setq snapshot
            (plist-put snapshot :root-focus-key
                       (or (plist-get (plist-get snapshot :root-node) :node_key)
                           (plist-get (plist-get snapshot :current-node) :node_key)))))
    (org-slipbox-buffer--transition-dedicated snapshot)))

(defun org-slipbox-buffer-set-compare-target (node)
  "Pin NODE as the dedicated buffer's comparison target."
  (interactive (list (org-slipbox-buffer--read-node-for-display)))
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (current-node (org-slipbox-buffer-session-current-node session)))
    (unless node
      (user-error "No comparison target selected"))
    (when (equal (plist-get node :node_key)
                 (plist-get current-node :node_key))
      (user-error "Choose a different note to compare"))
    (let ((snapshot (org-slipbox-buffer--history-snapshot session)))
      (setq snapshot (plist-put snapshot :compare-target node))
      (setq snapshot (plist-put snapshot :comparison-group 'all))
      (org-slipbox-buffer--transition-dedicated snapshot))))

(defun org-slipbox-buffer-clear-compare-target ()
  "Leave dedicated-buffer comparison mode."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session)))
    (unless (org-slipbox-buffer--comparison-active-p session)
      (user-error "No comparison target is pinned"))
    (setq snapshot (plist-put snapshot :compare-target nil))
    (setq snapshot (plist-put snapshot :comparison-group 'all))
    (org-slipbox-buffer--transition-dedicated snapshot)))

(defun org-slipbox-buffer-switch-comparison-group (group)
  "Switch the active comparison GROUP in a dedicated buffer."
  (interactive
   (list
    (intern
     (completing-read
      "Comparison group: "
      (mapcar #'symbol-name org-slipbox-buffer-comparison-groups)
      nil
      t
      nil
      nil
      (symbol-name (org-slipbox-buffer--current-comparison-group))))))
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session)))
    (unless (org-slipbox-buffer--comparison-active-p session)
      (user-error "No comparison target is pinned"))
    (unless (memq group org-slipbox-buffer-comparison-groups)
      (user-error "Unsupported comparison group %S" group))
    (setq snapshot (plist-put snapshot :comparison-group group))
    (org-slipbox-buffer--transition-dedicated snapshot)))

(defun org-slipbox-buffer-trail-add ()
  "Add the current dedicated cockpit state to the explicit trail."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session))
         (trail (copy-tree (org-slipbox-buffer-session-trail session)))
         (trail-index (org-slipbox-buffer-session-trail-index session))
         (active-trail (if (and trail trail-index)
                           (cl-subseq trail 0 (1+ trail-index))
                         trail))
         (last-step (car (last active-trail))))
    (unless (equal last-step snapshot)
      (setq active-trail (append active-trail (list snapshot))))
    (setf (org-slipbox-buffer-session-trail session) active-trail
          (org-slipbox-buffer-session-trail-index session)
          (and active-trail (1- (length active-trail))))
    (org-slipbox-buffer-render-contents)))

(defun org-slipbox-buffer-trail-back ()
  "Replay the previous explicit trail step."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (trail-index (org-slipbox-buffer-session-trail-index session)))
    (unless (and trail-index (> trail-index 0))
      (user-error "No earlier trail step"))
    (org-slipbox-buffer--replay-trail-at (1- trail-index))))

(defun org-slipbox-buffer-trail-forward ()
  "Replay the next explicit trail step."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (trail (org-slipbox-buffer-session-trail session))
         (trail-index (org-slipbox-buffer-session-trail-index session)))
    (unless (and trail-index (< trail-index (1- (length trail))))
      (user-error "No later trail step"))
    (org-slipbox-buffer--replay-trail-at (1+ trail-index))))

(defun org-slipbox-buffer-trail-clear ()
  "Clear the explicit trail for the current dedicated buffer."
  (interactive)
  (let ((session (org-slipbox-buffer--require-dedicated-session)))
    (unless (org-slipbox-buffer-session-trail session)
      (user-error "No active trail"))
    (setf (org-slipbox-buffer-session-trail session) nil
          (org-slipbox-buffer-session-trail-index session) nil)
    (org-slipbox-buffer-render-contents)))

(defun org-slipbox-buffer-save-artifact ()
  "Save the current dedicated cockpit state as a durable artifact."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (scope (org-slipbox-buffer--read-artifact-save-scope session))
         (title (org-slipbox-buffer--read-artifact-title session scope))
         (artifact-id (org-slipbox-buffer--read-artifact-id title))
         (artifact (org-slipbox-buffer--saved-artifact session scope artifact-id title)))
    (org-slipbox-buffer--confirm-artifact-overwrite artifact-id)
    (let* ((response (org-slipbox-rpc-save-exploration-artifact artifact))
           (saved (plist-get response :artifact)))
      (message "Saved exploration artifact %s" (plist-get saved :artifact_id))
      saved)))

(defun org-slipbox-buffer-load-artifact-by-id (artifact-id)
  "Load saved exploration artifact ARTIFACT-ID into the dedicated cockpit.
Return the executed artifact payload produced by the daemon."
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (response (org-slipbox-rpc-execute-exploration-artifact artifact-id))
         (executed (plist-get response :artifact)))
    (org-slipbox-buffer--restore-executed-artifact session executed)
    (message "Loaded exploration artifact %s" artifact-id)
    executed))

(defun org-slipbox-buffer-load-artifact ()
  "Load a saved exploration artifact into the current dedicated cockpit."
  (interactive)
  (let* ((summary (org-slipbox-buffer--read-artifact-summary))
         (artifact-id (plist-get summary :artifact_id)))
    (org-slipbox-buffer-load-artifact-by-id artifact-id)
    summary))

(defun org-slipbox-buffer-history-back ()
  "Move backward through dedicated-buffer navigation history."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (history (org-slipbox-buffer-session-history session)))
    (unless history
      (user-error "No earlier cockpit state"))
    (setf (org-slipbox-buffer-session-history session) (cdr history)
          (org-slipbox-buffer-session-future session)
          (cons (org-slipbox-buffer--history-snapshot session)
                (org-slipbox-buffer-session-future session)))
    (org-slipbox-buffer--apply-history-snapshot session (car history))
    (org-slipbox-buffer--reconcile-trail-position session)
    (org-slipbox-buffer-render-contents)))

(defun org-slipbox-buffer-history-forward ()
  "Move forward through dedicated-buffer navigation history."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (future (org-slipbox-buffer-session-future session)))
    (unless future
      (user-error "No later cockpit state"))
    (setf (org-slipbox-buffer-session-future session) (cdr future)
          (org-slipbox-buffer-session-history session)
          (cons (org-slipbox-buffer--history-snapshot session)
                (org-slipbox-buffer-session-history session)))
    (org-slipbox-buffer--apply-history-snapshot session (car future))
    (org-slipbox-buffer--reconcile-trail-position session)
    (org-slipbox-buffer-render-contents)))

(defun org-slipbox-buffer-toggle-frozen-context ()
  "Toggle whether dedicated exploration keeps its original root context."
  (interactive)
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (snapshot (org-slipbox-buffer--history-snapshot session))
         (frozen (not (org-slipbox-buffer-session-frozen-context session))))
    (setq snapshot (plist-put snapshot :frozen-context frozen))
    (unless frozen
      (setq snapshot (plist-put snapshot :root-node
                                (org-slipbox-buffer-session-current-node session)))
      (setq snapshot (plist-put snapshot :root-focus-key
                                (org-slipbox-buffer--current-focus-key session))))
    (when (and frozen (null (plist-get snapshot :root-node)))
      (setq snapshot (plist-put snapshot :root-node
                                (org-slipbox-buffer-session-current-node session)))
      (setq snapshot (plist-put snapshot :root-focus-key
                                (org-slipbox-buffer--current-focus-key session))))
    (org-slipbox-buffer--transition-dedicated snapshot)))

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

(defun org-slipbox-buffer--artifact-save-scope-choices (session)
  "Return saveable artifact scope choices for dedicated SESSION."
  (let ((choices
         (list
          (cons (if (org-slipbox-buffer--comparison-active-p session)
                    "Current comparison"
                  "Current lens view")
                'current))))
    (when (org-slipbox-buffer--trail-active-p session)
      (setq choices
            (append choices
                    '(("Current trail" . trail)
                      ("Current trail slice" . trail-slice)))))
    choices))

(defun org-slipbox-buffer--read-artifact-save-scope (session)
  "Read an artifact save scope for dedicated SESSION."
  (let ((choices (org-slipbox-buffer--artifact-save-scope-choices session)))
    (if (= (length choices) 1)
        (cdar choices)
      (cdr (assoc (completing-read "Save artifact from: "
                                   choices
                                   nil
                                   t
                                   nil
                                   nil
                                   (caar choices))
                  choices)))))

(defun org-slipbox-buffer--artifact-default-title (session scope)
  "Return a default durable artifact title for SESSION and save SCOPE."
  (let* ((snapshot (org-slipbox-buffer--history-snapshot session))
         (node (plist-get snapshot :current-node))
         (compare-target (plist-get snapshot :compare-target))
         (trail (org-slipbox-buffer--trail session))
         (trail-start (and trail (plist-get (car trail) :current-node)))
         (trail-index (or (org-slipbox-buffer--trail-position session) 0))
         (detached (org-slipbox-buffer--trail-detached-p session)))
    (pcase scope
      ('current
       (if compare-target
           (format "%s vs %s"
                   (plist-get node :title)
                   (plist-get compare-target :title))
         (format "%s (%s)"
                 (plist-get node :title)
                 (symbol-name (plist-get snapshot :active-lens)))))
      ('trail
       (format "Trail from %s (%s steps%s)"
               (plist-get trail-start :title)
               (length trail)
               (if detached " + branch" "")))
      ('trail-slice
       (format "Trail slice from %s (step %s of %s%s)"
               (plist-get trail-start :title)
               (1+ trail-index)
               (length trail)
               (if detached " + branch" "")))
      (_
       (user-error "Unsupported artifact save scope %S" scope)))))

(defun org-slipbox-buffer--artifact-default-id (title)
  "Return a stable default artifact identifier for TITLE."
  (let* ((slug (downcase (string-trim (or title ""))))
         (slug (replace-regexp-in-string "[^[:alnum:]]+" "-" slug))
         (slug (string-trim slug "-+" "-+")))
    (if (string-empty-p slug)
        "artifact"
      slug)))

(defun org-slipbox-buffer--read-artifact-title (session scope)
  "Prompt for a durable artifact title for SESSION and save SCOPE."
  (let* ((default (org-slipbox-buffer--artifact-default-title session scope))
         (value (read-string (format "Artifact title (%s): " default)
                             nil
                             nil
                             default)))
    (if (string-empty-p (string-trim value))
        default
      (string-trim value))))

(defun org-slipbox-buffer--read-artifact-id (title)
  "Prompt for a durable artifact identifier using TITLE as context."
  (let* ((default (org-slipbox-buffer--artifact-default-id title))
         (value (read-string (format "Artifact id (%s): " default)
                             nil
                             nil
                             default)))
    (if (string-empty-p (string-trim value))
        default
      (string-trim value))))

(defun org-slipbox-buffer--artifact-summaries ()
  "Return saved exploration artifact summaries through the daemon."
  (org-slipbox--plist-sequence
   (plist-get (org-slipbox-rpc-list-exploration-artifacts) :artifacts)))

(defun org-slipbox-buffer--artifact-summary-choice (summary)
  "Return a completing-read choice for saved artifact SUMMARY."
  (cons (format "%s [%s] <%s>"
                (plist-get summary :title)
                (plist-get summary :kind)
                (plist-get summary :artifact_id))
        summary))

(defun org-slipbox-buffer--read-artifact-summary ()
  "Read a saved exploration artifact summary."
  (let* ((summaries (org-slipbox-buffer--artifact-summaries))
         (choices (mapcar #'org-slipbox-buffer--artifact-summary-choice summaries)))
    (unless choices
      (user-error "No saved exploration artifacts"))
    (cdr (assoc (completing-read "Open artifact: " choices nil t nil nil (caar choices))
                choices))))

(defun org-slipbox-buffer--confirm-artifact-overwrite (artifact-id)
  "Prompt before overwriting saved ARTIFACT-ID."
  (when-let ((existing
              (seq-find (lambda (summary)
                          (equal (plist-get summary :artifact_id) artifact-id))
                        (org-slipbox-buffer--artifact-summaries))))
    (unless (y-or-n-p
             (format "Overwrite exploration artifact %s (%s)? "
                     artifact-id
                     (plist-get existing :title)))
      (user-error "Aborted artifact save"))))

(defun org-slipbox-buffer--required-node-key (node context)
  "Return NODE's required node key for CONTEXT, or signal a user error."
  (or (plist-get node :node_key)
      (user-error "Current %s does not have a node key" context)))

(defun org-slipbox-buffer--required-focus-key (snapshot key context)
  "Return SNAPSHOT's required focus KEY for CONTEXT, or signal a user error."
  (or (plist-get snapshot key)
      (org-slipbox-buffer--required-node-key
       (plist-get snapshot
                  (pcase key
                    (:root-focus-key :root-node)
                    (:current-focus-key :current-node)
                    (_ (user-error "Unsupported focus slot %S" key))))
       context)))

(defun org-slipbox-buffer--artifact-lens-symbol (value)
  "Return VALUE normalized as an exploration lens symbol."
  (if (symbolp value) value (intern value)))

(defun org-slipbox-buffer--artifact-comparison-group-symbol (value)
  "Return VALUE normalized as a comparison-group symbol."
  (if (symbolp value) value (intern (or value "all"))))

(defun org-slipbox-buffer--section-args (section)
  "Return SECTION argument plist, or nil for a bare function SECTION."
  (pcase section
    ((pred functionp) nil)
    (`(,_ . ,args) args)
    (_
     (user-error "Invalid org-slipbox buffer section specification: %S" section))))

(defun org-slipbox-buffer--structure-section-query-shape (section)
  "Return representable structure-query shape metadata for SECTION.
Signal a user error when SECTION changes the structure view in a way the
session and saved artifact models cannot encode faithfully."
  (let* ((function (org-slipbox-buffer--section-function section))
         (args (org-slipbox-buffer--section-args section))
         (allowed-filter-key
          (pcase function
            ('org-slipbox-buffer-backlinks-section :show-backlink-p)
            ('org-slipbox-buffer-forward-links-section :show-forward-link-p)
            (_
             (user-error "Unsupported structure section %S" function))))
         (explicit-unique-p nil)
         (unique nil)
         (explicit-limit-p nil)
         (limit org-slipbox-buffer-default-query-limit))
    (while args
      (let ((key (pop args))
            (value (pop args)))
        (pcase key
          (:unique
           (setq explicit-unique-p t)
           (setq unique (and value t)))
          (:limit
           (setq explicit-limit-p t)
           (setq limit value))
          (:section-heading nil)
          ((guard (eq key allowed-filter-key))
           (when value
             (user-error
              "Current structure lens plan cannot be saved faithfully: %S uses %S"
              function
              key)))
          (_
           (user-error
            "Current structure lens plan cannot be saved faithfully: %S uses unsupported option %S"
            function
            key)))))
    `(:function ,function
      :explicit-unique-p ,explicit-unique-p
      :unique ,unique
      :explicit-limit-p ,explicit-limit-p
      :limit ,limit)))

(defun org-slipbox-buffer--structure-plan-query-options (fallback-limit fallback-unique)
  "Return effective structure query options for the active dedicated plan.
FALLBACK-LIMIT and FALLBACK-UNIQUE supply the session-owned values when the
dedicated plan omits explicit structure-query modifiers."
  (let ((plan (org-slipbox-buffer--dedicated-section-plan 'structure))
        backlinks-shape
        forward-links-shape)
    (dolist (section plan)
      (pcase (org-slipbox-buffer--section-function section)
        ((or 'org-slipbox-buffer-node-section
             'org-slipbox-buffer-refs-section)
         nil)
        ('org-slipbox-buffer-backlinks-section
         (when backlinks-shape
           (user-error
            "Current structure lens plan cannot be saved faithfully: multiple backlink sections are not representable"))
         (setq backlinks-shape
               (org-slipbox-buffer--structure-section-query-shape section)))
        ('org-slipbox-buffer-forward-links-section
         (when forward-links-shape
           (user-error
            "Current structure lens plan cannot be saved faithfully: multiple forward-link sections are not representable"))
         (setq forward-links-shape
               (org-slipbox-buffer--structure-section-query-shape section)))
        (_
         (user-error
          "Current structure lens plan cannot be saved faithfully: section %S is not part of the saved structure lens model"
          (org-slipbox-buffer--section-function section)))))
    (unless (and backlinks-shape forward-links-shape)
      (user-error
       "Current structure lens plan cannot be saved faithfully: it must include exactly one backlinks section and one forward-links section"))
    (let* ((backlinks-options
            `(:limit ,(if (plist-get backlinks-shape :explicit-limit-p)
                          (plist-get backlinks-shape :limit)
                        fallback-limit)
              :unique ,(if (plist-get backlinks-shape :explicit-unique-p)
                           (plist-get backlinks-shape :unique)
                         fallback-unique)))
           (forward-links-options
            `(:limit ,(if (plist-get forward-links-shape :explicit-limit-p)
                          (plist-get forward-links-shape :limit)
                        fallback-limit)
              :unique ,(if (plist-get forward-links-shape :explicit-unique-p)
                           (plist-get forward-links-shape :unique)
                         fallback-unique))))
      (unless (equal backlinks-options forward-links-options)
        (user-error
         "Current structure lens plan cannot be saved faithfully: backlinks and forward links must use the same effective :unique and :limit"))
      backlinks-options)))

(defun org-slipbox-buffer--saved-lens-query-options (snapshot)
  "Return representable saved-query options for SNAPSHOT."
  (if (eq (plist-get snapshot :active-lens) 'structure)
      (org-slipbox-buffer--structure-plan-query-options
       (or (plist-get snapshot :query-limit)
           org-slipbox-buffer-default-query-limit)
       (and (plist-get snapshot :structure-unique) t))
    `(:limit ,(or (plist-get snapshot :query-limit)
                  org-slipbox-buffer-default-query-limit)
      :unique nil)))

(defun org-slipbox-buffer--saved-lens-view-artifact (snapshot)
  "Return a saved lens-view artifact plist from dedicated SNAPSHOT."
  (let ((lens (plist-get snapshot :active-lens))
        (query-options (org-slipbox-buffer--saved-lens-query-options snapshot)))
    `(:kind "lens-view"
      :root_node_key
      ,(org-slipbox-buffer--required-focus-key
        snapshot :root-focus-key "root focus")
      :current_node_key
      ,(org-slipbox-buffer--required-focus-key
        snapshot :current-focus-key "node focus")
      :lens ,(symbol-name lens)
      :limit ,(plist-get query-options :limit)
      :unique ,(org-slipbox-rpc--bool (plist-get query-options :unique))
      :frozen_context
      ,(org-slipbox-rpc--bool (plist-get snapshot :frozen-context)))))

(defun org-slipbox-buffer--validate-restored-snapshot (snapshot)
  "Validate that restored dedicated SNAPSHOT can replay faithfully here."
  (when (eq (plist-get snapshot :active-lens) 'structure)
    (let* ((requested-limit (or (plist-get snapshot :query-limit)
                                org-slipbox-buffer-default-query-limit))
           (requested-unique (and (plist-get snapshot :structure-unique) t))
           (effective
            (org-slipbox-buffer--structure-plan-query-options
             requested-limit
             requested-unique)))
      (unless (equal effective
                     `(:limit ,requested-limit :unique ,requested-unique))
        (user-error
         "Current structure lens plan cannot replay this artifact faithfully: saved and effective structure query semantics differ"))))
  snapshot)

(defun org-slipbox-buffer--snapshot-from-executed-lens-view (execution)
  "Return a dedicated snapshot restored from executed lens-view EXECUTION."
  (let ((artifact (plist-get execution :artifact)))
    (org-slipbox-buffer--validate-restored-snapshot
     `(:current-node ,(plist-get execution :current_note)
       :root-node ,(plist-get execution :root_note)
       :current-focus-key ,(plist-get artifact :current_node_key)
       :root-focus-key ,(plist-get artifact :root_node_key)
       :active-lens ,(org-slipbox-buffer--artifact-lens-symbol
                      (plist-get artifact :lens))
       :compare-target nil
       :comparison-group all
       :query-limit ,(plist-get artifact :limit)
       :structure-unique ,(and (plist-get artifact :unique) t)
       :frozen-context ,(plist-get artifact :frozen_context)))))

(defun org-slipbox-buffer--snapshot-from-executed-comparison (execution)
  "Return a dedicated snapshot restored from executed comparison EXECUTION."
  (let ((artifact (plist-get execution :artifact))
        (result (plist-get execution :result)))
    (org-slipbox-buffer--validate-restored-snapshot
     `(:current-node ,(plist-get result :left_note)
       :root-node ,(plist-get execution :root_note)
       :current-focus-key ,(plist-get (plist-get result :left_note) :node_key)
       :root-focus-key ,(plist-get (plist-get execution :root_note) :node_key)
       :active-lens ,(org-slipbox-buffer--artifact-lens-symbol
                      (plist-get artifact :active_lens))
       :compare-target ,(plist-get result :right_note)
       :comparison-group
       ,(org-slipbox-buffer--artifact-comparison-group-symbol
         (plist-get artifact :comparison_group))
       :query-limit ,(plist-get artifact :limit)
       :structure-unique ,(and (plist-get artifact :structure_unique) t)
       :frozen-context ,(plist-get artifact :frozen_context)))))

(defun org-slipbox-buffer--snapshot-from-executed-trail-step (step)
  "Return a dedicated snapshot restored from executed trail STEP."
  (pcase (plist-get step :kind)
    ("lens-view"
     (org-slipbox-buffer--snapshot-from-executed-lens-view step))
    ("comparison"
     (org-slipbox-buffer--snapshot-from-executed-comparison step))
    (_
     (user-error "Unsupported executed trail step kind %S"
                 (plist-get step :kind)))))

(defun org-slipbox-buffer--restore-trail-state (session replay)
  "Restore dedicated SESSION from executed trail REPLAY."
  (let* ((steps (mapcar #'org-slipbox-buffer--snapshot-from-executed-trail-step
                        (org-slipbox--plist-sequence (plist-get replay :steps))))
         (cursor (plist-get replay :cursor))
         (detached-step
          (when-let ((step (plist-get replay :detached_step)))
            (org-slipbox-buffer--snapshot-from-executed-trail-step step)))
         (current-snapshot (or detached-step (nth cursor steps))))
    (unless current-snapshot
      (user-error "Executed trail artifact did not yield a current cockpit state"))
    (setf (org-slipbox-buffer-session-history session) nil
          (org-slipbox-buffer-session-future session) nil
          (org-slipbox-buffer-session-trail session) steps
          (org-slipbox-buffer-session-trail-index session) cursor)
    (org-slipbox-buffer--apply-history-snapshot session current-snapshot)
    session))

(defun org-slipbox-buffer--restore-executed-artifact (session executed)
  "Restore dedicated SESSION from executed exploration artifact EXECUTED."
  (pcase (plist-get executed :kind)
    ("lens-view"
     (setf (org-slipbox-buffer-session-history session) nil
           (org-slipbox-buffer-session-future session) nil
           (org-slipbox-buffer-session-trail session) nil
           (org-slipbox-buffer-session-trail-index session) nil)
     (org-slipbox-buffer--apply-history-snapshot
      session
      (org-slipbox-buffer--snapshot-from-executed-lens-view executed)))
    ("comparison"
     (setf (org-slipbox-buffer-session-history session) nil
           (org-slipbox-buffer-session-future session) nil
           (org-slipbox-buffer-session-trail session) nil
           (org-slipbox-buffer-session-trail-index session) nil)
     (org-slipbox-buffer--apply-history-snapshot
      session
      (org-slipbox-buffer--snapshot-from-executed-comparison executed)))
    ("trail"
     (org-slipbox-buffer--restore-trail-state
      session
      (plist-get executed :replay)))
    (_
     (user-error "Unsupported executed artifact kind %S"
                 (plist-get executed :kind))))
  (org-slipbox-buffer-render-contents)
  session)

(defun org-slipbox-buffer--saved-comparison-artifact (snapshot)
  "Return a saved comparison artifact plist from dedicated SNAPSHOT."
  (let ((root-node (plist-get snapshot :root-node))
        (left-node (plist-get snapshot :current-node))
        (right-node (plist-get snapshot :compare-target)))
    `(:kind "comparison"
      :root_node_key ,(org-slipbox-buffer--required-node-key root-node "root node")
      :left_node_key ,(org-slipbox-buffer--required-node-key left-node "comparison source")
      :right_node_key ,(org-slipbox-buffer--required-node-key right-node "comparison target")
      :active_lens ,(symbol-name (plist-get snapshot :active-lens))
      :structure_unique
      ,(org-slipbox-rpc--bool (plist-get snapshot :structure-unique))
      :comparison_group ,(symbol-name (plist-get snapshot :comparison-group))
      :limit ,(or (plist-get snapshot :query-limit)
                  org-slipbox-buffer-default-query-limit)
      :frozen_context
      ,(org-slipbox-rpc--bool (plist-get snapshot :frozen-context)))))

(defun org-slipbox-buffer--saved-trail-step (snapshot)
  "Return a saved trail step plist from dedicated SNAPSHOT."
  (if (plist-get snapshot :compare-target)
      (org-slipbox-buffer--saved-comparison-artifact snapshot)
    (org-slipbox-buffer--saved-lens-view-artifact snapshot)))

(defun org-slipbox-buffer--saved-trail-artifact (session scope)
  "Return a saved trail artifact plist for SESSION and save SCOPE."
  (let* ((trail (copy-tree (org-slipbox-buffer--trail session)))
         (trail-index (org-slipbox-buffer--trail-position session)))
    (unless trail
      (user-error "No active trail to save"))
    (when (null trail-index)
      (user-error "Current trail does not have an active cursor"))
    (let* ((steps-snapshots (pcase scope
                              ('trail trail)
                              ('trail-slice (cl-subseq trail 0 (1+ trail-index)))
                              (_ (user-error "Unsupported trail save scope %S" scope))))
           (detached-step (and (org-slipbox-buffer--trail-detached-p session)
                               (org-slipbox-buffer--saved-trail-step
                                (org-slipbox-buffer--history-snapshot session)))))
      `(:kind "trail"
        :steps ,(mapcar #'org-slipbox-buffer--saved-trail-step steps-snapshots)
        :cursor ,(if (eq scope 'trail)
                     trail-index
                   (1- (length steps-snapshots)))
        :detached_step ,detached-step))))

(defun org-slipbox-buffer--saved-artifact-payload (session scope)
  "Return a saved artifact payload plist for dedicated SESSION and save SCOPE."
  (let ((snapshot (org-slipbox-buffer--history-snapshot session)))
    (pcase scope
      ('current
       (if (plist-get snapshot :compare-target)
           (org-slipbox-buffer--saved-comparison-artifact snapshot)
         (org-slipbox-buffer--saved-lens-view-artifact snapshot)))
      ((or 'trail 'trail-slice)
       (org-slipbox-buffer--saved-trail-artifact session scope))
      (_
       (user-error "Unsupported artifact save scope %S" scope)))))

(defun org-slipbox-buffer--saved-artifact (session scope artifact-id title)
  "Return a durable saved artifact plist.
SESSION supplies the current cockpit state, SCOPE selects what to save,
and ARTIFACT-ID with TITLE define durable metadata."
  (append `(:artifact_id ,artifact-id
            :title ,title)
          (org-slipbox-buffer--saved-artifact-payload session scope)))

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

(defun org-slipbox-buffer--forward-links (node &optional unique limit)
  "Return forward links for NODE.
When UNIQUE is non-nil, only return the first occurrence per destination
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'forward-links unique limit))

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

(defun org-slipbox-buffer--reflinks (node)
  "Return daemon-backed reflink matches for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'reflinks nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--unlinked-references (node)
  "Return daemon-backed unlinked references for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'unlinked-references nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--time-neighbors (node)
  "Return daemon-backed time neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'time 'time-neighbors nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--task-neighbors (node)
  "Return daemon-backed task neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'tasks 'task-neighbors nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--bridge-candidates (node)
  "Return daemon-backed bridge candidates for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'bridges 'bridge-candidates nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--dormant-notes (node)
  "Return daemon-backed dormant notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'dormant 'dormant-notes nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--unresolved-tasks (node)
  "Return daemon-backed unresolved tasks for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'unresolved-tasks nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--weakly-integrated-notes (node)
  "Return daemon-backed weakly integrated notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'weakly-integrated-notes nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--backlinks (node &optional unique limit)
  "Return backlinks for NODE.
When UNIQUE is non-nil, only return the first occurrence per source
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'backlinks unique limit))

(defun org-slipbox-buffer--exploration-section-kind-name (section-kind)
  "Return SECTION-KIND encoded for exploration results."
  (pcase section-kind
    ('backlinks "backlinks")
    ('forward-links "forward-links")
    ('reflinks "reflinks")
    ('unlinked-references "unlinked-references")
    ('time-neighbors "time-neighbors")
    ('task-neighbors "task-neighbors")
    ('bridge-candidates "bridge-candidates")
    ('dormant-notes "dormant-notes")
    ('unresolved-tasks "unresolved-tasks")
    ('weakly-integrated-notes "weakly-integrated-notes")
    (_
     (user-error "Unsupported exploration section kind %S" section-kind))))

(defun org-slipbox-buffer--exploration-cache-key (focus-key lens unique limit)
  "Return the cache key for FOCUS-KEY, exploration LENS, UNIQUE, and LIMIT."
  (list focus-key
        lens
        (or limit org-slipbox-buffer-default-query-limit)
        (and unique t)))

(defun org-slipbox-buffer--exploration-result (node lens &optional unique limit)
  "Return cached exploration results for NODE under LENS."
  (let ((limit (or limit org-slipbox-buffer-default-query-limit)))
    (when-let ((focus-key (or (org-slipbox-buffer--current-focus-key)
                              (plist-get node :node_key))))
    (if-let* ((session org-slipbox-buffer-session)
              (cache-key (org-slipbox-buffer--exploration-cache-key
                          focus-key lens unique limit))
              (cached (assoc cache-key
                             (org-slipbox-buffer-session-lens-cache session))))
        (cdr cached)
      (let ((result (org-slipbox-rpc-explore focus-key lens limit unique)))
        (when-let ((session org-slipbox-buffer-session))
          (let ((cache-key (org-slipbox-buffer--exploration-cache-key
                            focus-key lens unique limit))
                (existing (org-slipbox-buffer-session-lens-cache session)))
            (setf (org-slipbox-buffer-session-lens-cache session)
                  (cons (cons cache-key result)
                        (cl-remove cache-key existing :key #'car :test #'equal)))))
        result)))))

(defun org-slipbox-buffer--exploration-section-entries
    (node lens section-kind &optional unique limit)
  "Return SECTION-KIND entries for NODE under exploration LENS."
  (when-let* ((result (org-slipbox-buffer--exploration-result node lens unique limit))
              (sections (org-slipbox--plist-sequence (plist-get result :sections)))
              (section-name (org-slipbox-buffer--exploration-section-kind-name section-kind))
              (section (seq-find (lambda (candidate)
                                   (equal (plist-get candidate :kind) section-name))
                                 sections)))
    (org-slipbox--plist-sequence (plist-get section :entries))))

(defun org-slipbox-buffer--comparison-cache-key (left-node right-node limit)
  "Return the cache key for LEFT-NODE, RIGHT-NODE, and LIMIT."
  (list (plist-get left-node :node_key)
        (plist-get right-node :node_key)
        (or limit org-slipbox-buffer-default-query-limit)))

(defun org-slipbox-buffer--comparison-result (left-node right-node &optional limit)
  "Return cached comparison results for LEFT-NODE and RIGHT-NODE."
  (let ((limit (or limit (org-slipbox-buffer--current-query-limit))))
    (when-let ((left-key (plist-get left-node :node_key))
               (right-key (plist-get right-node :node_key)))
      (if-let* ((session org-slipbox-buffer-session)
                (cache-key (org-slipbox-buffer--comparison-cache-key
                            left-node right-node limit))
                (cached (assoc cache-key
                               (org-slipbox-buffer-session-comparison-cache session))))
          (cdr cached)
        (let ((result (org-slipbox-rpc-compare-notes left-key right-key limit)))
          (when-let ((session org-slipbox-buffer-session))
            (let ((cache-key (org-slipbox-buffer--comparison-cache-key
                              left-node right-node limit))
                  (existing (org-slipbox-buffer-session-comparison-cache session)))
              (setf (org-slipbox-buffer-session-comparison-cache session)
                    (cons (cons cache-key result)
                          (cl-remove cache-key existing :key #'car :test #'equal)))))
          result)))))

(provide 'org-slipbox-buffer)

;;; org-slipbox-buffer.el ends here
