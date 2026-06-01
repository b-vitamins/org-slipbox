;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

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

;; Context buffer for org-slipbox.

;;; Code:

(require 'cl-lib)
(require 'org-slipbox-buffer-artifact)
(require 'org-slipbox-buffer-render)
(require 'org-slipbox-buffer-state)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

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
    (org-slipbox-buffer-render-contents)
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

(defun org-slipbox-buffer--redisplay-h ()
  "Keep the persistent org-slipbox context buffer in sync with point."
  (when (and (get-buffer-window org-slipbox-buffer 'visible)
             (not (buffer-modified-p (or (buffer-base-buffer) (current-buffer)))))
    (org-slipbox-buffer-persistent-redisplay)))

(defun org-slipbox-buffer--persistent-cleanup-h ()
  "Clean up persistent buffer global state."
  (when (string= (buffer-name) org-slipbox-buffer)
    (org-slipbox-buffer-persistent-mode -1)))

(provide 'org-slipbox-buffer)

;;; org-slipbox-buffer.el ends here
