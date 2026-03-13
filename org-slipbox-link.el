;;; org-slipbox-link.el --- Link commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.2.0
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

;; Title-link and completion commands for `org-slipbox'.

;;; Code:

(require 'org)
(require 'subr-x)
(require 'org-slipbox-node)

(autoload 'org-slipbox--capture-node "org-slipbox-capture")

(defcustom org-slipbox-link-type "slipbox"
  "Org link type used for title-based org-slipbox links."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-link-auto-replace nil
  "When non-nil, replace `org-slipbox-link-type' links with `id:' links on save."
  :type 'boolean
  :group 'org-slipbox)

(defcustom org-slipbox-completion-everywhere nil
  "When non-nil, complete words at point into org-slipbox links."
  :type 'boolean
  :group 'org-slipbox)

(defconst org-slipbox-bracket-completion-re
  "\\[\\[\\(\\(?:slipbox:\\)?\\)\\([^]]*\\)]]"
  "Regexp for completion within Org bracket links.")

(org-link-set-parameters org-slipbox-link-type :follow #'org-slipbox-link-follow-link)

(define-minor-mode org-slipbox-completion-mode
  "Enable org-slipbox completion and link replacement in the current buffer."
  :lighter " Slipbox"
  (if org-slipbox-completion-mode
      (progn
        (add-hook 'completion-at-point-functions #'org-slipbox-complete-link-at-point nil t)
        (add-hook 'completion-at-point-functions #'org-slipbox-complete-everywhere nil t)
        (add-hook 'before-save-hook #'org-slipbox--replace-slipbox-links-on-save-h nil t))
    (remove-hook 'completion-at-point-functions #'org-slipbox-complete-link-at-point t)
    (remove-hook 'completion-at-point-functions #'org-slipbox-complete-everywhere t)
    (remove-hook 'before-save-hook #'org-slipbox--replace-slipbox-links-on-save-h t)))

(defun org-slipbox-link-follow-link (title-or-alias)
  "Visit the node named by TITLE-OR-ALIAS."
  (let ((node (org-slipbox-node-from-title-or-alias title-or-alias)))
    (org-mark-ring-push)
    (if node
        (progn
          (when org-slipbox-link-auto-replace
            (org-slipbox-link-replace-at-point))
          (org-slipbox--visit-node node))
      (let ((marker (point-marker))
            (auto-replace org-slipbox-link-auto-replace))
        (org-slipbox--capture-node
         title-or-alias
         nil
         nil
         nil
         `(:default-finalize
           ,(lambda (captured _session)
              (unwind-protect
                  (progn
                    (when (and auto-replace
                               (buffer-live-p (marker-buffer marker)))
                      (with-current-buffer (marker-buffer marker)
                        (save-excursion
                          (goto-char marker)
                          (org-slipbox-link-replace-at-point))))
                    (org-slipbox--visit-node captured))
                (set-marker marker nil)))
           :cleanup
           ,(lambda (_session)
              (when (markerp marker)
                (set-marker marker nil)))))))))

(defun org-slipbox-link-replace-at-point (&optional link)
  "Replace `org-slipbox-link-type' LINK at point with an `id:' link."
  (save-excursion
    (save-match-data
      (let* ((link (or link (org-element-context)))
             (type (org-element-property :type link))
             (path (org-element-property :path link))
             (description (and (org-element-property :contents-begin link)
                               (org-element-property :contents-end link)
                               (buffer-substring-no-properties
                                (org-element-property :contents-begin link)
                                (org-element-property :contents-end link))))
             node)
        (goto-char (org-element-property :begin link))
        (when (and (org-in-regexp org-link-any-re 1)
                   (string-equal type org-slipbox-link-type)
                   (setq node (save-match-data
                                (org-slipbox-node-from-title-or-alias path))))
          (let* ((node-with-id (org-slipbox--ensure-node-id node))
                 (explicit-id (plist-get node-with-id :explicit_id)))
            (replace-match (org-link-make-string
                            (concat "id:" explicit-id)
                            (or description path)))))))))

;;;###autoload
(defun org-slipbox-link-replace-all ()
  "Replace all `org-slipbox-link-type' links in the current buffer."
  (interactive)
  (org-with-point-at 1
    (while (search-forward (format "[[%s:" org-slipbox-link-type) nil t)
      (org-slipbox-link-replace-at-point))))

(defun org-slipbox-complete-link-at-point ()
  "Complete `org-slipbox-link-type' links at point."
  (let (slipbox-p start end)
    (when (org-in-regexp org-slipbox-bracket-completion-re 1)
      (setq slipbox-p (not (or (org-in-src-block-p)
                               (string-blank-p (match-string 1))))
            start (match-beginning 2)
            end (match-end 2))
      (list start end
            #'org-slipbox--title-completion-table
            :exit-function
            (lambda (string &rest _)
              (delete-char (- (length string)))
              (insert (concat (unless slipbox-p
                                (concat org-slipbox-link-type ":"))
                              string))
              (forward-char 2))))))

(defun org-slipbox-complete-everywhere ()
  "Complete words at point into org-slipbox links."
  (when (and org-slipbox-completion-everywhere
             (thing-at-point 'word)
             (not (org-in-src-block-p))
             (not (save-match-data (org-in-regexp org-link-any-re))))
    (let ((bounds (bounds-of-thing-at-point 'word)))
      (list (car bounds) (cdr bounds)
            #'org-slipbox--title-completion-table
            :exit-function
            (lambda (string _status)
              (delete-char (- (length string)))
              (insert "[[" org-slipbox-link-type ":" string "]]"))
            :exclusive 'no))))

(defun org-slipbox--replace-slipbox-links-on-save-h ()
  "Replace title-based org-slipbox links before saving when configured."
  (when org-slipbox-link-auto-replace
    (org-slipbox-link-replace-all)))

(provide 'org-slipbox-link)

;;; org-slipbox-link.el ends here
