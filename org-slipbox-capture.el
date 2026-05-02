;;; org-slipbox-capture.el --- Capture commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Public capture commands and high-level orchestration for `org-slipbox'.

;;; Code:

(require 'subr-x)
(require 'org-slipbox-capture-runtime)
(require 'org-slipbox-capture-session)
(require 'org-slipbox-capture-template)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

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

(defun org-slipbox--capture-node
    (title &optional template refs variables session)
  "Start a capture draft for TITLE using TEMPLATE, REFS, VARIABLES, and SESSION."
  (org-slipbox--capture-node-at-time title template refs nil variables session))

(defun org-slipbox--capture-node-at-time
    (title &optional template refs time variables session)
  "Start a capture draft for TITLE at TIME.
Use TEMPLATE, REFS, VARIABLES, and SESSION for initialization."
  (org-slipbox--capture-start title template refs time variables session))

(provide 'org-slipbox-capture)

;;; org-slipbox-capture.el ends here
