;;; org-slipbox-dailies.el --- Daily note commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Daily note commands for `org-slipbox'.

;;; Code:

(require 'cl-lib)
(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-dailies-directory "daily/"
  "Relative directory for daily notes inside `org-slipbox-directory'."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-dailies-file-format "%Y-%m-%d.org"
  "File name format for daily notes."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-dailies-title-format "%Y-%m-%d"
  "Title format for daily notes."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-dailies-entry-level 1
  "Heading level used for captured daily entries."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-dailies-find-file-hook nil
  "Hook run after visiting a daily note."
  :type 'hook
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-dailies-capture-today (heading)
  "Capture HEADING into today's daily note."
  (interactive (list (read-string "Daily entry: ")))
  (org-slipbox-dailies--capture (current-time) heading))

;;;###autoload
(defun org-slipbox-dailies-goto-today ()
  "Visit today's daily note, creating it if needed."
  (interactive)
  (org-slipbox-dailies--goto (current-time)))

;;;###autoload
(defun org-slipbox-dailies-capture-tomorrow (n heading)
  "Capture HEADING into the daily note N days in the future."
  (interactive
   (list (prefix-numeric-value current-prefix-arg)
         (read-string "Daily entry: ")))
  (org-slipbox-dailies--capture (org-slipbox-dailies--offset-time n) heading))

;;;###autoload
(defun org-slipbox-dailies-goto-tomorrow (n)
  "Visit the daily note N days in the future, creating it if needed."
  (interactive "p")
  (org-slipbox-dailies--goto (org-slipbox-dailies--offset-time n)))

;;;###autoload
(defun org-slipbox-dailies-capture-yesterday (n heading)
  "Capture HEADING into the daily note N days in the past."
  (interactive
   (list (prefix-numeric-value current-prefix-arg)
         (read-string "Daily entry: ")))
  (org-slipbox-dailies--capture (org-slipbox-dailies--offset-time (- n)) heading))

;;;###autoload
(defun org-slipbox-dailies-goto-yesterday (n)
  "Visit the daily note N days in the past, creating it if needed."
  (interactive "p")
  (org-slipbox-dailies--goto (org-slipbox-dailies--offset-time (- n))))

;;;###autoload
(defun org-slipbox-dailies-capture-date (&optional prefer-future)
  "Capture a heading into a daily note selected with the calendar.
With prefix argument PREFER-FUTURE, `org-read-date' prefers future dates."
  (interactive "P")
  (let ((time (org-slipbox-dailies--read-date "Capture to daily note: " prefer-future))
        (heading (read-string "Daily entry: ")))
    (org-slipbox-dailies--capture time heading)))

;;;###autoload
(defun org-slipbox-dailies-goto-date (&optional prefer-future)
  "Visit a daily note selected with the calendar, creating it if needed.
With prefix argument PREFER-FUTURE, `org-read-date' prefers future dates."
  (interactive "P")
  (org-slipbox-dailies--goto
   (org-slipbox-dailies--read-date "Find daily note: " prefer-future)))

;;;###autoload
(defun org-slipbox-dailies-goto-next-note (&optional n)
  "Visit the next daily note.
With numeric argument N, move N daily notes forward. Negative N moves
backward."
  (interactive "p")
  (unless (org-slipbox-dailies--daily-note-p)
    (user-error "Not in a daily note"))
  (let* ((n (or n 1))
         (dailies (org-slipbox-dailies--list-files))
         (current-file (org-slipbox-dailies--current-file))
         (position (cl-position current-file dailies :test #'string-equal))
         (target-index (and position (+ position n)))
         (note (and target-index (nth target-index dailies))))
    (unless position
      (user-error "Can't find current daily note file"))
    (unless note
      (if (>= n 0)
          (user-error "Already at newest note")
        (user-error "Already at oldest note")))
    (find-file note)
    (run-hooks 'org-slipbox-dailies-find-file-hook)
    note))

;;;###autoload
(defun org-slipbox-dailies-goto-previous-note (&optional n)
  "Visit the previous daily note.
With numeric argument N, move N daily notes backward. Negative N moves
forward."
  (interactive "p")
  (org-slipbox-dailies-goto-next-note (- (or n 1))))

;;;###autoload
(defun org-slipbox-dailies-find-directory ()
  "Visit `org-slipbox-dailies-directory'."
  (interactive)
  (let ((directory (expand-file-name org-slipbox-dailies-directory org-slipbox-directory)))
    (make-directory directory t)
    (find-file directory)))

(defun org-slipbox-dailies--goto (time)
  "Visit the daily note for TIME, creating it if needed."
  (let ((node (org-slipbox-dailies--ensure-note time)))
    (org-slipbox--visit-node node)
    (run-hooks 'org-slipbox-dailies-find-file-hook)
    node))

(defun org-slipbox-dailies--capture (time heading)
  "Capture HEADING into the daily note for TIME."
  (let ((heading (string-trim heading)))
    (when (string-empty-p heading)
      (user-error "Daily entry must not be empty"))
    (let ((node (org-slipbox-rpc-request
                 "slipbox/appendHeading"
                 `(:file_path ,(org-slipbox-dailies--path time)
                   :title ,(org-slipbox-dailies--title time)
                   :heading ,heading
                   :level ,org-slipbox-dailies-entry-level))))
      (org-slipbox--visit-node node)
      (run-hooks 'org-slipbox-dailies-find-file-hook)
      node)))

(defun org-slipbox-dailies--ensure-note (time)
  "Return the daily note node for TIME."
  (org-slipbox-rpc-request
  "slipbox/ensureFileNode"
  `(:file_path ,(org-slipbox-dailies--path time)
    :title ,(org-slipbox-dailies--title time))))

(defun org-slipbox-dailies--list-files (&rest extra-files)
  "Return daily note files, appending EXTRA-FILES."
  (let ((directory (expand-file-name org-slipbox-dailies-directory org-slipbox-directory)))
    (append
     (if (file-directory-p directory)
         (sort
          (seq-remove
           (lambda (file)
             (let ((name (file-name-nondirectory file)))
               (or (string-prefix-p "." name)
                   (auto-save-file-name-p name)
                   (backup-file-name-p name))))
           (directory-files-recursively directory "\\.org\\'"))
          #'string-lessp)
       nil)
     extra-files)))

(defun org-slipbox-dailies--daily-note-p (&optional file)
  "Return non-nil when FILE is a daily note."
  (when-let* ((candidate (or file (org-slipbox-dailies--current-file)))
              (path (expand-file-name candidate))
              (directory (expand-file-name org-slipbox-dailies-directory org-slipbox-directory)))
    (and (string-equal (downcase (or (file-name-extension path) "")) "org")
         (file-in-directory-p path directory))))

(defun org-slipbox-dailies--path (time)
  "Return the relative daily note path for TIME."
  (let ((file-name (format-time-string org-slipbox-dailies-file-format time)))
    (if (string-empty-p org-slipbox-dailies-directory)
        file-name
      (concat (file-name-as-directory org-slipbox-dailies-directory)
              file-name))))

(defun org-slipbox-dailies--title (time)
  "Return the daily note title for TIME."
  (format-time-string org-slipbox-dailies-title-format time))

(defun org-slipbox-dailies--offset-time (days)
  "Return a time DAYS away from now."
  (time-add (current-time) (days-to-time days)))

(defun org-slipbox-dailies--read-date (prompt prefer-future)
  "Read a daily note date with PROMPT.
When PREFER-FUTURE is non-nil, prefer future dates."
  (let ((org-read-date-prefer-future prefer-future))
    (org-read-date nil t nil prompt)))

(defun org-slipbox-dailies--current-file ()
  "Return the current base buffer file name."
  (buffer-file-name (buffer-base-buffer)))

(provide 'org-slipbox-dailies)

;;; org-slipbox-dailies.el ends here
