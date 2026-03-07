;;; org-slipbox-edit.el --- Structural editing commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Structural subtree editing commands for `org-slipbox'.

;;; Code:

(require 'org)
(require 'org-id)
(require 'subr-x)
(require 'org-slipbox-capture)
(require 'org-slipbox-metadata)
(require 'org-slipbox-node)

(defcustom org-slipbox-extract-file-name-template "${slug}.org"
  "Default relative file target template for `org-slipbox-extract-subtree'."
  :type 'string
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-refile (node)
  "Refile the current subtree under NODE."
  (interactive (list (org-slipbox--read-existing-node "Refile to: ")))
  (unless node
    (user-error "No target node selected"))
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (let* ((source-buffer (current-buffer))
         (source-file (expand-file-name buffer-file-name))
         (source-node (org-slipbox-node-at-point t))
         source-start
         source-end
         subtree-text
         target-buffer
         target-point)
    (when (equal (plist-get source-node :node_key)
                 (plist-get node :node_key))
      (user-error "Target is the same as current node"))
    (save-excursion
      (save-restriction
        (widen)
        (when (org-slipbox--file-node-p)
          (org-slipbox-demote-entire-buffer))
        (org-back-to-heading t)
        (setq source-start (copy-marker (point)))
        (setq source-end (copy-marker (save-excursion (org-end-of-subtree t t))))
        (setq subtree-text
              (buffer-substring-no-properties source-start source-end))))
    (setq target-buffer
          (find-file-noselect
           (expand-file-name (plist-get node :file_path) org-slipbox-directory)))
    (with-current-buffer target-buffer
      (save-excursion
        (save-restriction
          (widen)
          (setq target-point (org-slipbox--node-point node))
          (when (and (eq source-buffer target-buffer)
                     (>= target-point source-start)
                     (< target-point source-end))
            (user-error "Target is inside the current subtree"))
          (org-slipbox--paste-subtree-into-node node subtree-text))))
    (with-current-buffer source-buffer
      (save-excursion
        (save-restriction
          (widen)
          (org-preserve-local-variables
           (delete-region source-start source-end)))))
    (if (org-slipbox--buffer-empty-p source-buffer)
        (progn
          (when (buffer-live-p source-buffer)
            (with-current-buffer source-buffer
              (set-buffer-modified-p nil))
            (kill-buffer source-buffer))
          (delete-file source-file)
          (org-slipbox--sync-file-path source-file))
      (org-slipbox--save-and-sync-buffer source-buffer))
    (unless (eq source-buffer target-buffer)
      (org-slipbox--save-and-sync-buffer target-buffer))
    node))

;;;###autoload
(defun org-slipbox-extract-subtree (&optional file-path)
  "Extract the current subtree into FILE-PATH under `org-slipbox-directory'."
  (interactive)
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (save-excursion
    (save-restriction
      (widen)
      (when (org-slipbox--file-node-p)
        (user-error "Already a top-level node"))
      (org-back-to-heading t)
      (org-id-get-create)
      (let* ((title (nth 4 (org-heading-components)))
             (target-path (org-slipbox--extract-target-path title file-path))
             (source-buffer (current-buffer))
             (target-buffer nil)
             (source-start (copy-marker (point)))
             (source-end (copy-marker (save-excursion (org-end-of-subtree t t))))
             (subtree-text (buffer-substring-no-properties source-start source-end)))
        (when (file-exists-p target-path)
          (user-error "%s exists. Aborting" target-path))
        (setq target-buffer (find-file-noselect target-path))
        (org-preserve-local-variables
         (delete-region source-start source-end))
        (org-slipbox--save-and-sync-buffer source-buffer)
        (with-current-buffer target-buffer
          (save-excursion
            (save-restriction
              (widen)
              (erase-buffer)
              (org-mode)
              (org-slipbox--paste-subtree-into-node
               '(:kind "file" :line 1)
               subtree-text)
              (goto-char (point-min))
              (org-back-to-heading t)
              (while (> (org-current-level) 1)
                (org-promote-subtree))
              (org-slipbox--promote-entire-buffer-internal)))
          (org-slipbox--save-and-sync-buffer target-buffer))
        target-path))))

(defun org-slipbox--node-point (node)
  "Return the point for NODE in the current buffer."
  (save-excursion
    (goto-char (point-min))
    (forward-line (1- (plist-get node :line)))
    (point)))

(defun org-slipbox--paste-subtree-into-node (node subtree-text)
  "Paste SUBTREE-TEXT under NODE."
  (goto-char (org-slipbox--node-point node))
  (let ((kill-ring (list subtree-text))
        (kill-ring-yank-pointer nil))
    (setq kill-ring-yank-pointer kill-ring)
    (if (equal (plist-get node :kind) "file")
        (progn
          (goto-char (point-max))
          (unless (bolp)
            (newline))
          (org-paste-subtree 1 nil nil t))
      (org-back-to-heading t)
      (let ((level (org-get-valid-level (funcall outline-level) 1))
            (reversed (org-notes-order-reversed-p)))
        (goto-char
         (if reversed
             (or (outline-next-heading) (point-max))
           (or (save-excursion (org-get-next-sibling))
               (org-end-of-subtree t t)
               (point-max))))
        (unless (bolp)
          (newline))
        (org-paste-subtree level nil nil t))))
  (when org-auto-align-tags
    (let ((org-loop-over-headlines-in-active-region nil))
      (org-align-tags))))

(defun org-slipbox-demote-entire-buffer ()
  "Convert the current file note into a single top-level heading node."
  (interactive)
  (org-with-point-at 1
    (let ((title (org-slipbox--current-file-title))
          (tags (org-slipbox--file-tags)))
      (org-map-region #'org-do-demote (point-min) (point-max))
      (insert "* " title "\n")
      (org-back-to-heading)
      (when tags
        (org-set-tags tags))
      (org-slipbox--set-file-keyword "TITLE" nil)
      (org-slipbox--set-file-keyword "FILETAGS" nil))))

(defun org-slipbox--h1-count ()
  "Count level-1 headings in the current file."
  (let ((count 0))
    (org-with-wide-buffer
     (org-map-region
      (lambda ()
        (when (= (org-current-level) 1)
          (setq count (1+ count))))
      (point-min)
      (point-max)))
    count))

(defun org-slipbox--buffer-promoteable-p ()
  "Return non-nil when the current buffer can become a file node."
  (and (= (org-slipbox--h1-count) 1)
       (org-with-point-at 1 (org-at-heading-p))))

(defun org-slipbox-promote-entire-buffer ()
  "Convert a single level-1 heading buffer into a file node and sync it."
  (interactive)
  (org-slipbox--promote-entire-buffer-internal)
  (org-slipbox--save-and-sync-current-buffer))

(defun org-slipbox--promote-entire-buffer-internal ()
  "Convert the current single level-1 heading buffer into a file node."
  (unless (org-slipbox--buffer-promoteable-p)
    (user-error "Cannot promote: multiple root headings or extra file-level text"))
  (org-with-point-at 1
    (let ((title (nth 4 (org-heading-components)))
          (tags (org-get-tags)))
      (org-fold-show-all)
      (kill-whole-line)
      (org-slipbox--set-file-keyword "TITLE" title)
      (when tags
        (org-slipbox--set-file-tags tags))
      (org-map-region #'org-promote (point-min) (point-max)))))

(defun org-slipbox--extract-target-path (title &optional file-path)
  "Return an absolute extraction path for TITLE and FILE-PATH."
  (let* ((suggested (org-slipbox--expand-capture-template
                     org-slipbox-extract-file-name-template
                     title
                     (current-time)))
         (path (or file-path
                   (read-file-name
                    "Extract node to: "
                    (file-name-as-directory org-slipbox-directory)
                    suggested
                    nil
                    suggested)))
         (absolute (expand-file-name path org-slipbox-directory)))
    (unless (file-in-directory-p absolute (expand-file-name org-slipbox-directory))
      (user-error "%s is not inside org-slipbox-directory" absolute))
    (unless (string-suffix-p ".org" absolute)
      (user-error "Extracted file must end with .org"))
    absolute))

(defun org-slipbox--current-file-title ()
  "Return the current file node title."
  (or (org-slipbox--file-keyword-value "TITLE")
      (and buffer-file-name (file-name-base buffer-file-name))
      "note"))

(defun org-slipbox--buffer-empty-p (buffer)
  "Return non-nil when BUFFER has no meaningful content."
  (with-current-buffer buffer
    (string-empty-p
     (string-trim
      (buffer-substring-no-properties (point-min) (point-max))))))

(provide 'org-slipbox-edit)

;;; org-slipbox-edit.el ends here
