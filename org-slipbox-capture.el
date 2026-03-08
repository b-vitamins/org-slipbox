;;; org-slipbox-capture.el --- Capture commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Capture commands and template expansion helpers for `org-slipbox'.

;;; Code:

(require 'cl-lib)
(require 'org)
(require 'org-capture)
(require 'org-id)
(require 'subr-x)
(require 'org-slipbox-capture-template)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defvar org-slipbox-post-node-insert-hook nil
  "Hook run after `org-slipbox' inserts a new `id:' link.
Hook functions receive two arguments: the inserted ID and description.")

(cl-defstruct (org-slipbox-capture-session
               (:constructor org-slipbox--make-capture-session))
  "Transient org-slipbox capture-session metadata."
  title
  capture-title
  template
  template-options
  refs
  time
  target
  target-preview
  draft-kind
  capture-type
  initial-content
  target-file
  target-buffer-preexisting-p
  clock-marker
  caller-session)

(defvar-local org-slipbox-capture--session nil
  "Buffer-local org-slipbox capture session object.")

(defvar-local org-slipbox-capture--body-start nil
  "Marker pointing at the editable body of the current capture draft.")

(defvar org-slipbox-capture-current-session nil
  "Dynamically bound capture session for lifecycle handlers.")

(defvar org-slipbox-capture-current-node nil
  "Dynamically bound captured node for lifecycle handlers.")

(defvar org-slipbox-capture-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "C-c C-c") #'org-slipbox-capture-finalize)
    (define-key map (kbd "C-c C-k") #'org-slipbox-capture-abort)
    map)
  "Keymap used by `org-slipbox-capture-mode'.")

(define-derived-mode org-slipbox-capture-mode org-mode "Slipbox-Capture"
  "Major mode for transient org-slipbox capture drafts."
  (setq-local buffer-offer-save nil)
  (setq-local header-line-format
              '("org-slipbox draft  "
                "[C-c C-c] finalize  "
                "[C-c C-k] abort")))

;;;###autoload
(defun org-slipbox-capture (&optional title)
  "Create a new note with TITLE and visit it."
  (interactive)
  (let* ((title (or title (read-string "Capture title: ")))
         (template (org-slipbox--read-capture-template org-slipbox-capture-templates)))
    (org-slipbox--capture-node
     title
     template
     nil
     '(:default-finalize find-file))))

;;;###autoload
(defun org-slipbox-capture-ref (reference &optional title templates keys variables)
  "Visit the node for REFERENCE, or capture a new note with REFERENCE attached."
  (interactive (list (read-string "Ref: ")))
  (setq reference (string-trim reference))
  (when (string-empty-p reference)
    (user-error "Ref must not be empty"))
  (let* ((existing (org-slipbox-node-from-ref reference))
         (templates (or templates org-slipbox-capture-templates))
         (variables (plist-put (copy-sequence variables) :ref reference))
         (node (or existing
                   (org-slipbox--capture-node
                    (or title (read-string "Capture title: "))
                    (org-slipbox--read-capture-template templates keys)
                    (list reference)
                    variables
                    '(:default-finalize find-file)))))
    (when existing
      (org-slipbox--visit-node existing))
    node))

;;;###autoload
(defun org-slipbox-capture-to-node (node heading)
  "Capture HEADING under existing NODE and visit the captured child."
  (interactive
   (list (org-slipbox--read-existing-node "Capture to node: ")
         (read-string "Heading: ")))
  (unless node
    (user-error "No target node selected"))
  (let ((captured (org-slipbox-rpc-append-heading-to-node
                   (plist-get node :node_key)
                   heading)))
    (org-slipbox--visit-node captured)
    captured))

(defun org-slipbox--select-or-capture-node (query)
  "Return a node selected for QUERY, or create one."
  (let* ((choices (org-slipbox--search-node-choices query))
         (create-choice (format "[Create] %s" query))
         (collection (append choices (list (cons create-choice :create))))
         (selection (completing-read "Node: " collection nil t nil nil create-choice))
         (choice (cdr (assoc selection collection))))
    (cond
     ((eq choice :create) (org-slipbox--capture-node query))
     (choice choice)
     (t nil))))

(defun org-slipbox--capture-node (title &optional template refs variables session)
  "Start a capture draft for TITLE using TEMPLATE, REFS, VARIABLES, and SESSION."
  (org-slipbox--capture-node-at-time title template refs nil variables session))

(defun org-slipbox--capture-node-at-time
    (title &optional template refs time variables session)
  "Start a capture draft for TITLE using TEMPLATE, REFS, TIME, VARIABLES, and SESSION."
  (let* ((template (or template (org-slipbox--default-capture-template)))
         (template-options (org-slipbox--capture-template-options template))
         (session (copy-sequence session))
         (time (org-slipbox--capture-template-time time template-options))
         (capture-session nil))
    (org-slipbox--capture-validate-template-options
     template-options
     (org-slipbox--capture-template-type template))
    (setq capture-session
          (if (org-slipbox--typed-capture-template-p template)
              (org-slipbox--prepare-typed-capture-session
               title template refs time variables session)
            (org-slipbox--prepare-shorthand-capture-session
             title template refs time variables session)))
    (if (plist-get template-options :immediate-finish)
        (org-slipbox--capture-immediate-finish capture-session)
      (org-slipbox--open-capture-session capture-session))))

(defun org-slipbox--prepare-shorthand-capture-session
    (title template refs time variables session)
  "Return a shorthand capture session for TITLE with TEMPLATE.
REFS, TIME, VARIABLES, and SESSION describe the session state."
  (let* ((template-options (org-slipbox--capture-template-options template))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             time
                             variables)
                            title))
         (target (org-slipbox--expand-capture-target
                  template-options
                  title
                  time
                  variables))
         (target-file (org-slipbox--capture-target-file target)))
    (org-slipbox--make-capture-session
     :title title
     :capture-title capture-title
     :template template
     :template-options template-options
     :refs refs
     :time time
     :target target
     :target-preview (plist-get target :head)
     :draft-kind 'shorthand
     :capture-type (org-slipbox--capture-shorthand-type target)
     :initial-content ""
     :target-file target-file
     :target-buffer-preexisting-p (and target-file
                                      (org-slipbox--live-file-buffer target-file)
                                      t)
     :clock-marker (org-slipbox--capture-current-clock-marker)
     :caller-session session)))

(defun org-slipbox--prepare-typed-capture-session
    (title template refs time variables session)
  "Return an explicit typed capture session for TITLE with TEMPLATE.
REFS, TIME, VARIABLES, and SESSION describe the session state."
  (let* ((template-options (org-slipbox--capture-template-options template))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             time
                             variables)
                            title))
         (target (org-slipbox--expand-capture-target
                  template-options
                  capture-title
                  time
                  variables))
         (target-file (org-slipbox--capture-target-file target))
         (capture-type (org-slipbox--capture-template-type template))
         (content (org-slipbox--render-capture-body
                   (nth 3 template)
                   capture-type
                   capture-title
                   time
                   variables)))
    (org-slipbox--make-capture-session
     :title title
     :capture-title capture-title
     :template template
     :template-options template-options
     :refs refs
     :time time
     :target target
     :target-preview (plist-get target :head)
     :draft-kind 'typed
     :capture-type capture-type
     :initial-content content
     :target-file target-file
     :target-buffer-preexisting-p (and target-file
                                      (org-slipbox--live-file-buffer target-file)
                                      t)
     :clock-marker (org-slipbox--capture-current-clock-marker)
     :caller-session session)))

(defun org-slipbox--capture-create-buffer (capture-session)
  "Create and populate a draft buffer for CAPTURE-SESSION."
  (let ((buffer (generate-new-buffer
                 (format "*org-slipbox capture: %s*"
                         (org-slipbox-capture-session-capture-title capture-session)))))
    (with-current-buffer buffer
      (org-slipbox-capture-mode)
      (setq-local org-slipbox-capture--session capture-session)
      (let ((inhibit-read-only t))
        (org-slipbox--capture-insert-session-header capture-session)
        (setq-local org-slipbox--capture-body-start (point-marker))
        (insert (org-slipbox-capture-session-initial-content capture-session)))
      (goto-char (if (string-empty-p (org-slipbox-capture-session-initial-content capture-session))
                     org-slipbox--capture-body-start
                   (point-max)))
      (set-buffer-modified-p nil))
    buffer))

(defun org-slipbox--open-capture-session (capture-session)
  "Display a draft buffer for CAPTURE-SESSION and return that buffer."
  (let ((buffer (org-slipbox--capture-create-buffer capture-session)))
    (pop-to-buffer buffer)
    buffer))

(defun org-slipbox--capture-immediate-finish (capture-session)
  "Commit CAPTURE-SESSION immediately without displaying a draft buffer."
  (let ((buffer (org-slipbox--capture-create-buffer capture-session)))
    (unwind-protect
        (with-current-buffer buffer
          (org-slipbox--capture-finalize-buffer))
      (when (buffer-live-p buffer)
        (org-slipbox--capture-kill-buffer buffer)))))

(defun org-slipbox--capture-insert-session-header (capture-session)
  "Insert a read-only metadata header for CAPTURE-SESSION."
  (let* ((header-start (point))
         (header (org-slipbox--capture-session-header capture-session)))
    (insert header)
    (add-text-properties
     header-start
     (point)
     '(read-only t
       front-sticky t
       rear-nonsticky (read-only)
       face shadow))))

(defun org-slipbox--capture-session-header (capture-session)
  "Return the read-only header text for CAPTURE-SESSION."
  (string-join
   (append
    (list "# org-slipbox capture draft"
          "# C-c C-c finalize  C-c C-k abort"
          (format "# Title: %s"
                  (org-slipbox-capture-session-capture-title capture-session))
          (format "# Target: %s"
                  (org-slipbox--capture-target-summary
                   (org-slipbox-capture-session-target capture-session)))
          (format "# Type: %s"
                  (symbol-name
                   (org-slipbox-capture-session-capture-type capture-session))))
    (when-let ((refs (org-slipbox-capture-session-refs capture-session)))
      (list (format "# Refs: %s" (string-join refs ", "))))
    (let ((finalize
           (org-slipbox--capture-resolve-finalize
            (org-slipbox-capture-session-template-options capture-session)
            (org-slipbox-capture-session-caller-session capture-session))))
      (list (format "# Finalize: %s"
                    (org-slipbox--capture-finalize-summary finalize))))
    (when-let ((preview (org-slipbox-capture-session-target-preview capture-session)))
      (append
       '("# Preview:")
       (mapcar (lambda (line)
                 (if (string-empty-p line)
                     "#"
                   (format "#   %s" line)))
               (split-string preview "\n"))
       '("#")))
    '(""))
   "\n"))

(defun org-slipbox--capture-target-summary (target)
  "Return a human-readable summary for capture TARGET."
  (pcase (plist-get target :kind)
    ('node
     (format "node %s" (plist-get target :node_key)))
    ((or 'file 'file+olp)
     (let ((file (or (plist-get target :file_path) "<auto>"))
           (outline-path (plist-get target :outline_path)))
       (if outline-path
         (format "%s :: %s" file (string-join outline-path " / "))
         file)))
    (_
     (format "%S" target))))

(defun org-slipbox--capture-current-clock-marker ()
  "Return a snapshot marker for the currently active Org clock, or nil."
  (and (boundp 'org-clock-marker)
       (markerp org-clock-marker)
       (marker-buffer org-clock-marker)
       (copy-marker org-clock-marker)))

(defun org-slipbox--capture-finalize-summary (finalize)
  "Return a human-readable summary for FINALIZE."
  (cond
   ((null finalize) "none")
   ((symbolp finalize) (symbol-name finalize))
   ((functionp finalize) "custom")
   (t (format "%S" finalize))))

;;;###autoload
(defun org-slipbox-capture-finalize ()
  "Commit the current org-slipbox draft through the Rust RPC layer."
  (interactive)
  (org-slipbox--capture-finalize-buffer))

(defun org-slipbox--capture-finalize-buffer ()
  "Commit the current org-slipbox draft buffer through the Rust RPC layer."
  (unless (org-slipbox-capture-session-p org-slipbox-capture--session)
    (user-error "Not in an org-slipbox capture draft"))
  (let* ((capture-session org-slipbox-capture--session)
         (template-options (org-slipbox-capture-session-template-options capture-session))
         (caller-session (org-slipbox-capture-session-caller-session capture-session))
         (buffer (current-buffer)))
    (let* ((content
            (progn
              (org-slipbox--capture-run-phase-functions
               :prepare-finalize
               template-options
               capture-session
               nil)
              (buffer-substring-no-properties org-slipbox--capture-body-start
                                              (point-max))))
           (node (org-slipbox--capture-materialize-session
                  capture-session
                  content
                  caller-session)))
      (unwind-protect
          (progn
            (org-slipbox--capture-run-phase-functions
             :before-finalize
             template-options
             capture-session
             node)
            (org-slipbox--capture-kill-buffer buffer)
            (org-slipbox--capture-run-lifecycle-with-session
             node
             template-options
             caller-session
             capture-session))
        (org-slipbox--capture-cleanup-session caller-session))
      node)))

;;;###autoload
(defun org-slipbox-capture-abort ()
  "Abort the current org-slipbox draft without mutating target files."
  (interactive)
  (unless (org-slipbox-capture-session-p org-slipbox-capture--session)
    (user-error "Not in an org-slipbox capture draft"))
  (let ((capture-session org-slipbox-capture--session))
    (org-slipbox--capture-cleanup-session
     (org-slipbox-capture-session-caller-session capture-session))
    (org-slipbox--capture-kill-buffer (current-buffer))
    (message "Aborted org-slipbox capture")
    nil))

(defun org-slipbox--capture-kill-buffer (buffer)
  "Kill BUFFER without save prompts."
  (when (buffer-live-p buffer)
    (with-current-buffer buffer
      (set-buffer-modified-p nil)
      (let ((kill-buffer-query-functions nil))
        (kill-buffer buffer)))))

(defun org-slipbox--capture-cleanup-session (session)
  "Release transient marker state owned by SESSION."
  (when-let ((cleanup (plist-get session :cleanup)))
    (funcall cleanup session))
  (when-let ((marker (plist-get session :call-location)))
    (when (markerp marker)
      (set-marker marker nil)))
  (when-let ((region (plist-get session :region)))
    (when (markerp (car region))
      (set-marker (car region) nil))
    (when (markerp (cdr region))
      (set-marker (cdr region) nil))))

(defun org-slipbox--capture-commit-session (capture-session content)
  "Commit CAPTURE-SESSION with editable CONTENT through the generic RPC."
  (org-slipbox-rpc-capture-template
   (org-slipbox--capture-session-params capture-session content)))

(defun org-slipbox--capture-materialize-session
    (capture-session content caller-session)
  "Materialize CAPTURE-SESSION CONTENT and return the captured node.
CALLER-SESSION is used to resolve lifecycle-dependent preview behavior."
  (if (org-slipbox--capture-use-preview-p capture-session)
      (org-slipbox--capture-preview-session
       capture-session
       content
       caller-session)
    (org-slipbox--capture-save-session capture-session content)))

(defun org-slipbox--capture-use-preview-p (capture-session)
  "Return non-nil when CAPTURE-SESSION should use unsaved preview materialization."
  (let ((template-options (org-slipbox-capture-session-template-options capture-session)))
    (and (plist-get template-options :no-save)
         (not (and (plist-get template-options :kill-buffer)
                   (not (org-slipbox-capture-session-target-buffer-preexisting-p
                         capture-session)))))))

(defun org-slipbox--capture-save-session (capture-session content)
  "Persist CAPTURE-SESSION CONTENT through the daemon and refresh live buffers."
  (org-slipbox--capture-preflight-target-buffer capture-session)
  (let ((node (org-slipbox--capture-commit-session capture-session content)))
    (org-slipbox--capture-refresh-target-buffer capture-session node)
    node))

(defun org-slipbox--capture-preview-session
    (capture-session content caller-session)
  "Apply an unsaved preview for CAPTURE-SESSION CONTENT and return the node.
CALLER-SESSION is used to determine whether preview materialization must
assign an explicit Org ID."
  (let* ((ensure-node-id
          (eq (org-slipbox--capture-resolve-finalize
               (org-slipbox-capture-session-template-options capture-session)
               caller-session)
              'insert-link))
         (preview
          (org-slipbox-rpc-capture-template-preview
           (org-slipbox--capture-preview-params
            capture-session
            content
            ensure-node-id)))
         (node (plist-get preview :node)))
    (when (and ensure-node-id
               (not (plist-get node :explicit_id)))
      (user-error "No-save capture preview did not assign an explicit Org ID"))
    (org-slipbox--capture-apply-preview capture-session preview)
    (org-slipbox--capture-register-preview-node node)
    node))

(defun org-slipbox--capture-session-params (capture-session content)
  "Return generic capture RPC params for CAPTURE-SESSION and CONTENT."
  (let* ((template-options (org-slipbox-capture-session-template-options capture-session))
         (target (org-slipbox-capture-session-target capture-session))
         (empty-lines (org-slipbox--capture-template-empty-lines template-options)))
    (append
     (list :title (org-slipbox-capture-session-capture-title capture-session)
           :capture_type
           (symbol-name (org-slipbox-capture-session-capture-type capture-session))
           :content content
           :prepend (and (plist-get template-options :prepend) t)
           :empty_lines_before (plist-get empty-lines :before)
           :empty_lines_after (plist-get empty-lines :after))
     (when-let ((table-line-pos (plist-get template-options :table-line-pos)))
       (list :table_line_pos table-line-pos))
     (when-let ((refs (org-slipbox-capture-session-refs capture-session)))
       (list :refs refs))
     (org-slipbox--capture-target-params target))))

(defun org-slipbox--capture-preview-params
    (capture-session content ensure-node-id)
  "Return no-save preview RPC params for CAPTURE-SESSION and CONTENT.
When ENSURE-NODE-ID is non-nil, the daemon must assign an explicit Org ID
within the preview content before returning."
  (append
   (org-slipbox--capture-session-params capture-session content)
   (when-let ((source-override
               (org-slipbox--capture-preview-source-override capture-session)))
     (list :source_override source-override))
   (when ensure-node-id
     (list :ensure_node_id t))))

(defun org-slipbox--capture-target-file (target)
  "Return an absolute file path for TARGET when one is known."
  (when-let ((file-path (plist-get target :file_path)))
    (expand-file-name file-path org-slipbox-directory)))

(defun org-slipbox--capture-preview-source-override (capture-session)
  "Return current live target contents for CAPTURE-SESSION, or nil."
  (when-let* ((target-file (org-slipbox-capture-session-target-file capture-session))
              (buffer (org-slipbox--live-file-buffer target-file)))
    (with-current-buffer buffer
      (save-restriction
        (widen)
        (buffer-substring-no-properties (point-min) (point-max))))))

(defun org-slipbox--capture-preflight-target-buffer (capture-session)
  "Save and sync live target buffers for CAPTURE-SESSION before writing."
  (when-let ((target-file (org-slipbox-capture-session-target-file capture-session)))
    (org-slipbox--sync-live-file-buffer-if-needed target-file)))

(defun org-slipbox--capture-refresh-target-buffer (capture-session node)
  "Refresh any live target buffer for CAPTURE-SESSION after writing NODE."
  (when-let ((target-file (or (org-slipbox-capture-session-target-file capture-session)
                              (when-let ((file-path (plist-get node :file_path)))
                                (expand-file-name file-path org-slipbox-directory)))))
    (org-slipbox--refresh-live-file-buffer target-file)))

(defun org-slipbox--capture-apply-preview (capture-session preview)
  "Apply PREVIEW content into the live target buffer for CAPTURE-SESSION."
  (let* ((relative-path (plist-get preview :file_path))
         (target-file (expand-file-name relative-path org-slipbox-directory))
         (buffer (or (org-slipbox--live-file-buffer target-file)
                     (find-file-noselect target-file))))
    (setf (org-slipbox-capture-session-target-file capture-session) target-file)
    (with-current-buffer buffer
      (let ((inhibit-read-only t))
        (save-restriction
          (widen)
          (erase-buffer)
          (insert (plist-get preview :content))))
      (set-buffer-modified-p t))
    buffer))

(defun org-slipbox--capture-register-preview-node (node)
  "Register preview NODE with `org-id' compatibility state when possible."
  (when-let ((explicit-id (plist-get node :explicit_id))
             (file-path (plist-get node :file_path)))
    (org-id-add-location explicit-id
                         (expand-file-name file-path org-slipbox-directory))))


(defun org-slipbox--capture-run-lifecycle (node template-options session)
  "Apply template and caller lifecycle settings for NODE."
  (org-slipbox--capture-run-lifecycle-with-session
   node template-options session nil))

(defun org-slipbox--capture-run-lifecycle-with-session
    (node template-options session capture-session)
  "Apply template and caller lifecycle settings for NODE and CAPTURE-SESSION."
  (let ((finalize (org-slipbox--capture-resolve-finalize template-options session)))
    (org-slipbox--capture-run-phase-functions
     :after-finalize
     template-options
     capture-session
     node)
    (when finalize
      (org-slipbox--capture-call-finalizer finalize node session))
    (org-slipbox--capture-handle-clock
     node
     template-options
     capture-session)
    (org-slipbox--capture-handle-kill-buffer
     node
     template-options
     capture-session)
    node))

(defun org-slipbox--capture-handle-kill-buffer (node template-options capture-session)
  "Honor `:kill-buffer' for NODE from TEMPLATE-OPTIONS and CAPTURE-SESSION."
  (when (and capture-session
             (plist-get template-options :kill-buffer)
             (not (org-slipbox-capture-session-target-buffer-preexisting-p capture-session)))
    (when-let ((target-file (or (org-slipbox-capture-session-target-file capture-session)
                                (when-let ((file-path (plist-get node :file_path)))
                                  (expand-file-name file-path org-slipbox-directory)))))
      (org-slipbox--kill-live-file-buffer target-file))))

(defun org-slipbox--capture-handle-clock (node template-options capture-session)
  "Honor clock-related TEMPLATE-OPTIONS for NODE and CAPTURE-SESSION."
  (let ((clock-marker (and capture-session
                           (org-slipbox-capture-session-clock-marker capture-session))))
    (when (and (plist-get template-options :clock-in)
               (not (plist-get template-options :clock-keep)))
      (org-slipbox--capture-clock-node node))
    (cond
     ((plist-get template-options :clock-keep)
      (when clock-marker
        (org-slipbox--capture-resume-clock clock-marker)))
     ((plist-get template-options :clock-resume)
      (when clock-marker
        (org-slipbox--capture-resume-clock clock-marker))))))

(defun org-slipbox--capture-clock-node (node)
  "Start an Org clock on NODE."
  (let ((buffer (find-file-noselect
                 (expand-file-name (plist-get node :file_path) org-slipbox-directory))))
    (with-current-buffer buffer
      (goto-char (point-min))
      (forward-line (1- (plist-get node :line)))
      (when (derived-mode-p 'org-mode)
        (ignore-errors (org-back-to-heading t)))
      (org-clock-in))))

(defun org-slipbox--capture-resume-clock (clock-marker)
  "Resume the Org clock snapshot at CLOCK-MARKER."
  (when (and (markerp clock-marker)
             (marker-buffer clock-marker))
    (with-current-buffer (marker-buffer clock-marker)
      (goto-char clock-marker)
      (when (derived-mode-p 'org-mode)
        (ignore-errors (org-back-to-heading t)))
      (org-clock-in))))

(defun org-slipbox--capture-run-phase-functions
    (phase template-options capture-session node)
  "Run lifecycle PHASE handlers from TEMPLATE-OPTIONS.
CAPTURE-SESSION and NODE are exposed through dynamic variables."
  (dolist (function (org-slipbox--capture-phase-functions
                     (plist-get template-options phase)))
    (let ((org-slipbox-capture-current-session capture-session)
          (org-slipbox-capture-current-node node))
      (funcall function))))

(defun org-slipbox--capture-phase-functions (value)
  "Return lifecycle functions normalized from VALUE."
  (cond
   ((null value) nil)
   ((functionp value) (list value))
   ((listp value) value)
   (t nil)))

(defun org-slipbox--capture-resolve-finalize (template-options session)
  "Resolve the effective finalize action for TEMPLATE-OPTIONS and SESSION."
  (or (plist-get session :finalize)
      (plist-get template-options :finalize)
      (and (plist-get template-options :jump-to-captured) 'find-file)
      (plist-get session :default-finalize)))

(defun org-slipbox--capture-call-finalizer (finalize node session)
  "Run FINALIZE for NODE with SESSION."
  (cond
   ((symbolp finalize)
    (let ((function (intern-soft
                     (format "org-slipbox--capture-finalize-%s" finalize))))
      (unless (functionp function)
        (user-error "Unsupported capture finalize action %S" finalize))
      (funcall function node session)))
   ((functionp finalize)
    (funcall finalize node session))
   (t
    (user-error "Unsupported capture finalize action %S" finalize))))

(defun org-slipbox--capture-finalize-find-file (node _session)
  "Visit NODE after capture."
  (org-slipbox--visit-node node))

(defun org-slipbox--capture-finalize-insert-link (node session)
  "Insert a link to NODE using SESSION caller context."
  (let ((marker (plist-get session :call-location)))
    (unless marker
      (user-error "No caller location available for insert-link finalize"))
    (let ((buffer (marker-buffer marker)))
      (unless (buffer-live-p buffer)
        (user-error "The caller buffer for insert-link is no longer live"))
      (with-current-buffer buffer
        (goto-char marker)
        (when-let ((region (plist-get session :region)))
          (delete-region (car region) (cdr region))
          (goto-char (car region))
          (set-marker (car region) nil)
          (set-marker (cdr region) nil))
        (let* ((node-with-id (org-slipbox--ensure-node-id node))
               (description (or (plist-get session :link-description)
                                (org-slipbox-node-formatted node-with-id))))
          (insert (format "[[id:%s][%s]]"
                          (plist-get node-with-id :explicit_id)
                          description))
          (run-hook-with-args 'org-slipbox-post-node-insert-hook
                              (plist-get node-with-id :explicit_id)
                              description))
        (set-marker marker nil)))))

(provide 'org-slipbox-capture)

;;; org-slipbox-capture.el ends here
