;;; org-slipbox-capture-runtime.el --- Capture lifecycle for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.6.1
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

;; Internal capture finalize, preview, and lifecycle helpers for `org-slipbox'.

;;; Code:

(require 'org)
(require 'org-id)
(require 'subr-x)
(require 'org-slipbox-capture-session)
(require 'org-slipbox-capture-template)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defvar org-slipbox-post-node-insert-hook nil
  "Hook run after `org-slipbox' inserts a new `id:' link.
Hook functions receive two arguments: the inserted ID and description.")

(defvar org-slipbox-capture-current-session nil
  "Dynamically bound capture session for lifecycle handlers.")

(defvar org-slipbox-capture-current-node nil
  "Dynamically bound captured node for lifecycle handlers.")

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
         (node (plist-get preview :preview_node)))
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
           :prepend (org-slipbox-rpc--bool (plist-get template-options :prepend))
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

(provide 'org-slipbox-capture-runtime)

;;; org-slipbox-capture-runtime.el ends here
