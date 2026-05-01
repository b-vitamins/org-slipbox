;;; org-slipbox-agenda.el --- Agenda commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.5.0
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

;; Agenda commands backed by the indexed org-slipbox store.

;;; Code:

(require 'button)
(require 'org)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

;;;###autoload
(defun org-slipbox-agenda-today ()
  "Show indexed agenda entries for today."
  (interactive)
  (org-slipbox-agenda-date (current-time)))

;;;###autoload
(defun org-slipbox-agenda-date (&optional time prefer-future)
  "Show indexed agenda entries for TIME.
When called interactively without TIME, prompt for a date.
With prefix argument PREFER-FUTURE, `org-read-date' prefers future dates."
  (interactive (list nil current-prefix-arg))
  (let* ((time (or time
                   (let ((org-read-date-prefer-future prefer-future))
                     (org-read-date nil t nil "Agenda date: "))))
         (range (org-slipbox-agenda--day-range time))
         (response (org-slipbox-rpc-agenda (car range) (cdr range)))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes))))
    (with-current-buffer (get-buffer-create "*org-slipbox agenda*")
      (let ((inhibit-read-only t))
        (erase-buffer)
        (special-mode)
        (insert (format "Agenda for %s\n\n" (format-time-string "%Y-%m-%d" time)))
        (if nodes
            (dolist (node nodes)
              (org-slipbox-agenda--insert-node node)
              (insert "\n"))
          (insert "No agenda entries.\n")))
      (display-buffer (current-buffer)))))

(defun org-slipbox-agenda--day-range (time)
  "Return the inclusive ISO day range for TIME."
  (cons (format-time-string "%Y-%m-%dT00:00:00" time)
        (format-time-string "%Y-%m-%dT23:59:59" time)))

(defun org-slipbox-agenda--insert-node (node)
  "Insert NODE into the current agenda buffer."
  (let ((start (point))
        (timing (org-slipbox-agenda--timing node)))
    (insert (org-slipbox--node-display node))
    (when timing
      (insert " | " timing))
    (make-text-button
     start
     (point)
     'follow-link t
     'action (lambda (_button)
               (org-slipbox--visit-node node)))))

(defun org-slipbox-agenda--timing (node)
  "Return a compact timing summary for NODE."
  (string-join
   (delq nil
         (list
          (when-let ((scheduled (plist-get node :scheduled_for)))
            (format "SCHEDULED %s" (substring scheduled 0 10)))
          (when-let ((deadline (plist-get node :deadline_for)))
            (format "DEADLINE %s" (substring deadline 0 10)))))
   " "))

(provide 'org-slipbox-agenda)

;;; org-slipbox-agenda.el ends here
