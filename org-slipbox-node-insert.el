;;; org-slipbox-node-insert.el --- Node insertion for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.9.0
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

;; Link insertion helpers for `org-slipbox' nodes.

;;; Code:

(require 'seq)
(require 'org)
(require 'org-slipbox-node-read)
(require 'org-slipbox-rpc)

(autoload 'org-slipbox--capture-node "org-slipbox-capture")
(defvar org-slipbox-post-node-insert-hook)
(defvar org-slipbox-capture-templates)

(defconst org-slipbox--node-insert-capture-types
  '(plain entry item checkitem table-line)
  "Capture content types recognized by `org-slipbox-node-insert'.")

(defun org-slipbox--node-insert-typed-template-p (template)
  "Return non-nil when TEMPLATE uses the explicit typed capture syntax."
  (memq (nth 2 template) org-slipbox--node-insert-capture-types))

(defun org-slipbox--node-insert-template-with-immediate-finish (template)
  "Return TEMPLATE rewritten to commit directly without opening a draft."
  (let* ((template (copy-tree template))
         (prefix-length (if (org-slipbox--node-insert-typed-template-p template) 4 2))
         (prefix (seq-take template prefix-length))
         (options (nthcdr prefix-length template)))
    (append prefix (plist-put options :immediate-finish t))))

(defun org-slipbox-node-insert (&optional initial-input filter-fn)
  "Insert an `id:' link to a selected node.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes."
  (interactive)
  (unwind-protect
      (atomic-change-group
        (let* ((region (org-slipbox--node-insert-region))
               (description (plist-get region :text))
               (initial-input (or initial-input description))
               (node (org-slipbox-node-read initial-input filter-fn nil nil "Node: ")))
          (when node
            (if (plist-get node :file_path)
                (let* ((node-with-id (org-slipbox--ensure-node-id node))
                       (description (or description
                                        (org-slipbox-node-formatted node-with-id))))
                  (org-slipbox-node-insert-link node-with-id description region)
                  node-with-id)
              (org-slipbox--capture-node
               (plist-get node :title)
               nil
               nil
               nil
               `(:finalize insert-link
                 :call-location ,(point-marker)
                 :link-description ,description
                 :region ,(and region
                               (cons (plist-get region :beg)
                                     (plist-get region :end)))))))))
    (deactivate-mark)))

;;;###autoload
(defun org-slipbox-node-insert-immediate (&optional initial-input filter-fn)
  "Insert an `id:' link and commit newly captured nodes immediately.
INITIAL-INPUT seeds the minibuffer. FILTER-FN filters indexed nodes."
  (interactive)
  (let ((org-slipbox-capture-templates
         (mapcar #'org-slipbox--node-insert-template-with-immediate-finish
                 org-slipbox-capture-templates)))
    (org-slipbox-node-insert initial-input filter-fn)))

(defun org-slipbox--ensure-node-id (node)
  "Return NODE with an explicit ID, assigning one if needed."
  (if (plist-get node :explicit_id)
      node
    (org-slipbox-rpc-ensure-node-id (plist-get node :node_key))))

(defun org-slipbox--node-insert-region ()
  "Return the active region details for node insertion, or nil."
  (when (use-region-p)
    (let ((beg (set-marker (make-marker) (region-beginning)))
          (end (set-marker (make-marker) (region-end))))
      (list :beg beg
            :end end
            :text (org-link-display-format
                   (buffer-substring-no-properties beg end))))))

(defun org-slipbox--replace-node-insert-region (region link)
  "Replace REGION with LINK, or insert LINK at point when REGION is nil."
  (unwind-protect
      (progn
        (when region
          (goto-char (plist-get region :beg))
          (delete-region (plist-get region :beg) (plist-get region :end)))
        (insert link))
    (when region
      (set-marker (plist-get region :beg) nil)
      (set-marker (plist-get region :end) nil))))

(defun org-slipbox-node-insert-link (node description &optional region)
  "Insert a link to NODE using DESCRIPTION, replacing REGION when present."
  (let ((id (plist-get node :explicit_id)))
    (org-slipbox--replace-node-insert-region
     region
     (format "[[id:%s][%s]]" id description))
    (run-hook-with-args 'org-slipbox-post-node-insert-hook id description)))

(defalias 'org-slipbox--insert-node-link #'org-slipbox-node-insert-link)

(provide 'org-slipbox-node-insert)

;;; org-slipbox-node-insert.el ends here
