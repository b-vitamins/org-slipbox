;;; org-slipbox-protocol.el --- org-protocol integration for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.4.0
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

;; Explicit org-protocol integration for `org-slipbox'.

;;; Code:

(require 'ol)
(require 'org-protocol)
(require 'subr-x)
(require 'org-slipbox-capture)
(require 'org-slipbox-node)

(defvar org-protocol-protocol-alist)
(defvar org-stored-links)

(defcustom org-slipbox-protocol-store-links nil
  "When non-nil, store links handled by `org-slipbox-protocol-mode'."
  :type 'boolean
  :group 'org-slipbox)

(defconst org-slipbox-protocol--handlers
  '(("org-slipbox-ref" :protocol "roam-ref" :function org-slipbox-protocol-open-ref)
    ("org-slipbox-node" :protocol "roam-node" :function org-slipbox-protocol-open-node))
  "Protocol handler entries installed by `org-slipbox-protocol-mode'.")

(define-minor-mode org-slipbox-protocol-mode
  "Register `org-slipbox' handlers with `org-protocol'.

This mode owns protocol registration explicitly instead of mutating
`org-protocol-protocol-alist' at load time."
  :global t
  :group 'org-slipbox
  (require 'org-protocol)
  (if org-slipbox-protocol-mode
      (org-slipbox-protocol--register-handlers)
    (org-slipbox-protocol--unregister-handlers)))

(defun org-slipbox-protocol-open-ref (info)
  "Open or capture the node referenced by protocol INFO."
  (require 'org-protocol)
  (let* ((info (org-slipbox-protocol--decode-info info))
         (reference (plist-get info :ref))
         (title (or (plist-get info :title) reference))
         (body (or (plist-get info :body) ""))
         (template (plist-get info :template))
         (annotation (org-link-make-string reference title)))
    (unless reference
      (user-error "No ref key provided"))
    (when org-slipbox-protocol-store-links
      (push (list reference title) org-stored-links))
    (org-link-store-props
     :type (org-slipbox-protocol--plain-link-type reference)
     :link reference
     :annotation annotation
     :initial body)
    (raise-frame)
    (org-slipbox-capture-ref
     reference
     title
     org-slipbox-capture-ref-templates
     template
     (list :ref reference
           :body body
           :annotation annotation
           :link reference))
    nil))

(defun org-slipbox-protocol-open-node (info)
  "Visit the node identified by protocol INFO."
  (let* ((info (org-slipbox-protocol--decode-info info))
         (id (plist-get info :node)))
    (unless id
      (user-error "No node key provided"))
    (raise-frame)
    (org-slipbox--visit-node
     (or (org-slipbox-node-from-id id)
         (user-error "No node with ID %s" id)))
    nil))

(defun org-slipbox-protocol--register-handlers ()
  "Register protocol handlers idempotently."
  (dolist (handler (reverse org-slipbox-protocol--handlers))
    (unless (assoc (car handler) org-protocol-protocol-alist)
      (push handler org-protocol-protocol-alist))))

(defun org-slipbox-protocol--unregister-handlers ()
  "Remove protocol handlers."
  (dolist (handler org-slipbox-protocol--handlers)
    (setq org-protocol-protocol-alist
          (assq-delete-all (car handler) org-protocol-protocol-alist))))

(defun org-slipbox-protocol--decode-info (info)
  "Decode string values in protocol INFO."
  (let (decoded)
    (while info
      (let* ((key (car info))
             (value (cadr info))
             (decoded-value
              (if (stringp value)
                  (org-link-decode
                   (if (eq key :ref)
                       (org-protocol-sanitize-uri value)
                     value))
                value)))
        (setq decoded (plist-put decoded key decoded-value)
              info (cddr info))))
    decoded))

(defun org-slipbox-protocol--plain-link-type (reference)
  "Return the plain link type for REFERENCE, or nil."
  (and (string-match org-link-plain-re reference)
       (match-string 1 reference)))

(provide 'org-slipbox-protocol)

;;; org-slipbox-protocol.el ends here
