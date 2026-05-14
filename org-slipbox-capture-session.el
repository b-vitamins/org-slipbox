;;; org-slipbox-capture-session.el --- Capture draft sessions for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.13.1
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

;; Internal draft-session helpers for `org-slipbox' capture flows.

;;; Code:

(require 'cl-lib)
(require 'org)
(require 'subr-x)
(require 'org-slipbox-capture-template)
(require 'org-slipbox-node)

(declare-function org-slipbox--capture-finalize-buffer "org-slipbox-capture-runtime")
(declare-function org-slipbox--capture-kill-buffer "org-slipbox-capture-runtime")
(declare-function org-slipbox--capture-resolve-finalize "org-slipbox-capture-runtime")
(declare-function org-slipbox-capture-abort "org-slipbox-capture-runtime")
(declare-function org-slipbox-capture-finalize "org-slipbox-capture-runtime")

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

(defvar-local org-slipbox--capture-body-start nil
  "Marker pointing at the editable body of the current capture draft.")

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

(defun org-slipbox--capture-start
    (title &optional template refs time variables session)
  "Start a capture draft for TITLE.
Use TEMPLATE, REFS, TIME, VARIABLES, and SESSION for initialization."
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

(defun org-slipbox--capture-target-file (target)
  "Return an absolute file path for TARGET when one is known."
  (when-let ((file-path (plist-get target :file_path)))
    (expand-file-name file-path org-slipbox-directory)))

(provide 'org-slipbox-capture-session)

;;; org-slipbox-capture-session.el ends here
