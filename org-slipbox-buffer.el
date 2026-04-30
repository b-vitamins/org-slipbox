;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.3.0
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

(defcustom org-slipbox-buffer-sections
  (list #'org-slipbox-buffer-node-section
        #'org-slipbox-buffer-refs-section
        #'org-slipbox-buffer-backlinks-section
        #'org-slipbox-buffer-forward-links-section
        #'org-slipbox-buffer-reflinks-section
        #'org-slipbox-buffer-unlinked-references-section
        #'org-slipbox-buffer-time-neighbors-section
        #'org-slipbox-buffer-task-neighbors-section
        #'org-slipbox-buffer-bridge-candidates-section
        #'org-slipbox-buffer-dormant-notes-section
        #'org-slipbox-buffer-unresolved-tasks-section
        #'org-slipbox-buffer-weakly-integrated-notes-section)
  "Section specifications rendered by `org-slipbox-buffer-render-contents'.

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

(defconst org-slipbox-buffer-lenses
  '(structure refs time tasks bridges dormant unresolved)
  "Declared exploration lenses supported by the dedicated buffer.")

(defconst org-slipbox-buffer-comparison-groups '(all overlap divergence tension)
  "Declared comparison groups supported by the dedicated buffer.")

(cl-defstruct org-slipbox-buffer-session
  "Explicit session state for an org-slipbox context buffer."
  kind
  current-node
  root-node
  active-lens
  compare-target
  comparison-group
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
                                (org-slipbox-buffer-session-current-node session))))
    (when (and frozen (null (plist-get snapshot :root-node)))
      (setq snapshot (plist-put snapshot :root-node
                                (org-slipbox-buffer-session-current-node session))))
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
  "Render configured sections for NODE."
  (dolist (section org-slipbox-buffer-sections)
    (when (org-slipbox-buffer--section-allowed-p section node)
      (org-slipbox-buffer--render-section section node))))

(defun org-slipbox-buffer--section-allowed-p (section node)
  "Return non-nil when SECTION should render for NODE."
  (and (org-slipbox-buffer--section-visible-p section)
       (or (null org-slipbox-buffer-section-filter-function)
           (funcall org-slipbox-buffer-section-filter-function section node))))

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
     (user-error "Invalid `org-slipbox-buffer-sections' specification: %S" section))))

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

(defun org-slipbox-buffer--section-visible-p (section)
  "Return non-nil when SECTION belongs in the current buffer mode."
  (let ((lens (org-slipbox-buffer--section-lens section)))
    (cond
     ((null lens) t)
     ((org-slipbox-buffer--dedicated-p)
      (eq lens (org-slipbox-buffer--current-lens)))
     (t
      (memq lens '(structure refs))))))

(defun org-slipbox-buffer--make-persistent-session (&optional node)
  "Return a persistent context-buffer session for NODE."
  (make-org-slipbox-buffer-session
   :kind 'persistent
   :current-node node
   :root-node node))

(defun org-slipbox-buffer--normalize-persistent-session (session node)
  "Normalize persistent SESSION around NODE.
Dedicated-only state must not survive on the persistent tracking path."
  (setf (org-slipbox-buffer-session-kind session) 'persistent
        (org-slipbox-buffer-session-current-node session) node
        (org-slipbox-buffer-session-root-node session) node
        (org-slipbox-buffer-session-active-lens session) nil
        (org-slipbox-buffer-session-compare-target session) nil
        (org-slipbox-buffer-session-comparison-group session) nil
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
   :active-lens 'structure
   :comparison-group 'all
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

(defun org-slipbox-buffer--section-lens (section)
  "Return the declared exploration lens for SECTION, or nil."
  (pcase (org-slipbox-buffer--section-function section)
    ('org-slipbox-buffer-node-section nil)
    ('org-slipbox-buffer-refs-section 'refs)
    ('org-slipbox-buffer-backlinks-section 'structure)
    ('org-slipbox-buffer-forward-links-section 'structure)
    ('org-slipbox-buffer-reflinks-section 'refs)
    ('org-slipbox-buffer-unlinked-references-section 'refs)
    ('org-slipbox-buffer-time-neighbors-section 'time)
    ('org-slipbox-buffer-task-neighbors-section 'tasks)
    ('org-slipbox-buffer-bridge-candidates-section 'bridges)
    ('org-slipbox-buffer-dormant-notes-section 'dormant)
    ('org-slipbox-buffer-unresolved-tasks-section 'unresolved)
    ('org-slipbox-buffer-weakly-integrated-notes-section 'unresolved)
    (_ nil)))

(defun org-slipbox-buffer--current-lens (&optional session)
  "Return the active exploration lens for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-active-lens session)))

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

(defun org-slipbox-buffer--trail-attached-p (&optional session)
  "Return non-nil when SESSION or the current buffer is on its trail cursor."
  (let* ((session (or session org-slipbox-buffer-session))
         (trail (and session (org-slipbox-buffer-session-trail session)))
         (trail-index (and session (org-slipbox-buffer-session-trail-index session))))
    (and trail
         trail-index
         (equal (nth trail-index trail)
                (org-slipbox-buffer--history-snapshot session)))))

(defun org-slipbox-buffer--history-snapshot (&optional session)
  "Return a navigation snapshot for SESSION or the current buffer."
  (let ((session (or session org-slipbox-buffer-session)))
    (list :current-node (org-slipbox-buffer-session-current-node session)
          :root-node (org-slipbox-buffer-session-root-node session)
          :active-lens (org-slipbox-buffer-session-active-lens session)
          :compare-target (org-slipbox-buffer-session-compare-target session)
          :comparison-group (org-slipbox-buffer--current-comparison-group session)
          :frozen-context (org-slipbox-buffer-session-frozen-context session))))

(defun org-slipbox-buffer--apply-history-snapshot (session snapshot)
  "Apply SNAPSHOT to SESSION and clear its transient caches."
  (setf (org-slipbox-buffer-session-current-node session)
        (plist-get snapshot :current-node)
        (org-slipbox-buffer-session-root-node session)
        (plist-get snapshot :root-node)
        (org-slipbox-buffer-session-active-lens session)
        (plist-get snapshot :active-lens)
        (org-slipbox-buffer-session-compare-target session)
        (plist-get snapshot :compare-target)
        (org-slipbox-buffer-session-comparison-group session)
        (plist-get snapshot :comparison-group)
        (org-slipbox-buffer-session-frozen-context session)
        (plist-get snapshot :frozen-context)
        (org-slipbox-buffer-session-lens-cache session) nil
        (org-slipbox-buffer-session-comparison-cache session) nil))

(defun org-slipbox-buffer--transition-dedicated (snapshot)
  "Apply dedicated-buffer SNAPSHOT as a navigable transition."
  (let* ((session (org-slipbox-buffer--require-dedicated-session))
         (current (org-slipbox-buffer--history-snapshot session)))
    (unless (equal current snapshot)
      (setf (org-slipbox-buffer-session-history session)
            (cons current (org-slipbox-buffer-session-history session))
            (org-slipbox-buffer-session-future session) nil)
      (org-slipbox-buffer--apply-history-snapshot session snapshot)
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
  (dolist (entry (org-slipbox-buffer--trail-entries))
    (org-slipbox-buffer--insert-trail-entry entry)
    (insert "\n"))
  (insert "\n"))

(defun org-slipbox-buffer--trail-entries ()
  "Return decorated entries for the explicit trail."
  (let ((trail (org-slipbox-buffer--trail))
        (trail-index (org-slipbox-buffer--trail-position))
        (trail-attached (org-slipbox-buffer--trail-attached-p))
        (index 0)
        entries)
    (dolist (snapshot trail (nreverse entries))
      (push (list :index index
                  :snapshot snapshot
                  :current (eq index trail-index)
                  :attached trail-attached)
            entries)
      (setq index (1+ index)))))

(defun org-slipbox-buffer--insert-trail-entry (entry)
  "Insert one explicit trail ENTRY."
  (let* ((index (plist-get entry :index))
         (snapshot (plist-get entry :snapshot))
         (label (org-slipbox-buffer--trail-label snapshot))
         (prefix (cond
                  ((plist-get entry :current)
                   (if (plist-get entry :attached) "=> " "~> "))
                  (t "   "))))
    (insert prefix)
    (insert-text-button
     (format "%s. %s" (1+ index) label)
     'follow-link t
     'help-echo "Replay this trail step"
     'action (lambda (_button)
               (org-slipbox-buffer--replay-trail-at index)))))

(defun org-slipbox-buffer--trail-label (snapshot)
  "Return a short label for trail SNAPSHOT."
  (let* ((node (plist-get snapshot :current-node))
         (compare-target (plist-get snapshot :compare-target))
         (lens (plist-get snapshot :active-lens))
         (group (plist-get snapshot :comparison-group))
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
    (string-join parts "  |  ")))

(defun org-slipbox-buffer--render-comparison-sections (comparison)
  "Render COMPARISON sections for the active comparison group."
  (let* ((left-note (plist-get comparison :left_note))
         (right-note (plist-get comparison :right_note))
         (group (org-slipbox-buffer--current-comparison-group))
         (sections (org-slipbox--plist-sequence (plist-get comparison :sections))))
    (dolist (section sections)
      (when (org-slipbox-buffer--comparison-section-visible-p
             group
             (plist-get section :kind))
        (org-slipbox-buffer--insert-occurrence-section
         (org-slipbox-buffer--comparison-section-heading section left-note right-note)
         (org-slipbox--plist-sequence (plist-get section :entries))
         (org-slipbox-buffer--comparison-empty-message section)
         #'org-slipbox-buffer--insert-comparison-entry)))))

(defun org-slipbox-buffer--comparison-section-visible-p (group kind)
  "Return non-nil when comparison section KIND belongs in GROUP."
  (or (eq group 'all)
      (eq group (org-slipbox-buffer--comparison-section-group kind))))

(defun org-slipbox-buffer--comparison-section-group (kind)
  "Return the comparison group for section KIND."
  (pcase kind
    ((or "shared-refs" "shared-backlinks" "shared-forward-links") 'overlap)
    ((or "left-only-refs" "right-only-refs") 'divergence)
    ("indirect-connectors" 'tension)
    (_ nil)))

(defun org-slipbox-buffer--comparison-section-heading (section left-note right-note)
  "Return the rendered heading for SECTION between LEFT-NOTE and RIGHT-NOTE."
  (pcase (plist-get section :kind)
    ("shared-refs" "Shared Refs")
    ("left-only-refs"
     (format "Refs only in %s" (plist-get left-note :title)))
    ("right-only-refs"
     (format "Refs only in %s" (plist-get right-note :title)))
    ("shared-backlinks" "Shared Backlinks")
    ("shared-forward-links" "Shared Forward Links")
    ("indirect-connectors" "Indirect Connectors")
    (_
     (user-error "Unsupported comparison section kind %S"
                 (plist-get section :kind)))))

(defun org-slipbox-buffer--comparison-empty-message (section)
  "Return the empty-message string for comparison SECTION."
  (pcase (plist-get section :kind)
    ("shared-refs" "No shared refs found.")
    ("left-only-refs" "No left-only refs found.")
    ("right-only-refs" "No right-only refs found.")
    ("shared-backlinks" "No shared backlinks found.")
    ("shared-forward-links" "No shared forward links found.")
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
    (node &key unique show-backlink-p (section-heading "Backlinks") (limit 200))
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
    (node &key unique show-forward-link-p (section-heading "Forward Links") (limit 200))
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
    (setq snapshot (plist-put snapshot :root-node root-node))
    (org-slipbox-buffer--transition-dedicated snapshot)))

(defun org-slipbox-buffer--explanation-string (entry)
  "Return a display string for ENTRY's explanation payload."
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
      ("bridge-candidate"
       (format "shared ref: %s via %s"
               (plist-get explanation :reference)
               (plist-get explanation :via_title)))
      ("dormant-shared-reference"
       (format "shared ref: %s, older untouched material"
               (plist-get explanation :reference)))
      ("unresolved-shared-reference"
       (format "shared ref: %s, task state: %s"
               (plist-get explanation :reference)
               (plist-get explanation :todo_keyword)))
      ("weakly-integrated-shared-reference"
       (format "shared ref: %s, structural links: %s"
               (plist-get explanation :reference)
               (plist-get explanation :structural_link_count)))
      ("unlinked-reference"
       (format "unlinked mention: %s"
               (plist-get explanation :matched_text)))
      ("shared-scheduled-date"
       (format "shared scheduled date: %s" (plist-get explanation :date)))
      ("shared-deadline-date"
       (format "shared deadline date: %s" (plist-get explanation :date)))
      ("shared-todo-keyword"
       (format "shared task state: %s"
               (plist-get explanation :todo_keyword))))))

(defun org-slipbox-buffer--insert-explanation (entry)
  "Insert ENTRY's explanation payload when it is present."
  (when-let ((reason (org-slipbox-buffer--explanation-string entry)))
    (insert " "
            (propertize reason 'face 'italic))))

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
   node 'refs 'reflinks nil 200))

(defun org-slipbox-buffer--unlinked-references (node)
  "Return daemon-backed unlinked references for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'unlinked-references nil 200))

(defun org-slipbox-buffer--time-neighbors (node)
  "Return daemon-backed time neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'time 'time-neighbors nil 200))

(defun org-slipbox-buffer--task-neighbors (node)
  "Return daemon-backed task neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'tasks 'task-neighbors nil 200))

(defun org-slipbox-buffer--bridge-candidates (node)
  "Return daemon-backed bridge candidates for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'bridges 'bridge-candidates nil 200))

(defun org-slipbox-buffer--dormant-notes (node)
  "Return daemon-backed dormant notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'dormant 'dormant-notes nil 200))

(defun org-slipbox-buffer--unresolved-tasks (node)
  "Return daemon-backed unresolved tasks for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'unresolved-tasks nil 200))

(defun org-slipbox-buffer--weakly-integrated-notes (node)
  "Return daemon-backed weakly integrated notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'weakly-integrated-notes nil 200))

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

(defun org-slipbox-buffer--exploration-cache-key (lens unique limit)
  "Return the cache key for exploration LENS, UNIQUE, and LIMIT."
  (list lens (or limit 200) (and unique t)))

(defun org-slipbox-buffer--exploration-result (node lens &optional unique limit)
  "Return cached exploration results for NODE under LENS."
  (let ((limit (or limit 200)))
    (when-let ((node-key (plist-get node :node_key)))
    (if-let* ((session org-slipbox-buffer-session)
              (cache-key (org-slipbox-buffer--exploration-cache-key lens unique limit))
              (cached (assoc cache-key
                             (org-slipbox-buffer-session-lens-cache session))))
        (cdr cached)
      (let ((result (org-slipbox-rpc-explore node-key lens limit unique)))
        (when-let ((session org-slipbox-buffer-session))
          (let ((cache-key (org-slipbox-buffer--exploration-cache-key lens unique limit))
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
        (or limit 200)))

(defun org-slipbox-buffer--comparison-result (left-node right-node &optional limit)
  "Return cached comparison results for LEFT-NODE and RIGHT-NODE."
  (let ((limit (or limit 200)))
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
