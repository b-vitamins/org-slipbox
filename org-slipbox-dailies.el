;;; org-slipbox-dailies.el --- Daily note commands for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.7.0
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
(require 'org-slipbox-capture)
(require 'org-slipbox-files)
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

(defcustom org-slipbox-dailies-capture-templates
  nil
  "Capture templates for daily notes.
When non-nil, templates use the same format as
`org-slipbox-capture-templates' and override the legacy
`org-slipbox-dailies-entry-level' capture flow."
  :type 'sexp
  :group 'org-slipbox)

(defcustom org-slipbox-dailies-find-file-hook nil
  "Hook run after visiting a daily note."
  :type 'hook
  :group 'org-slipbox)

(defface org-slipbox-dailies-calendar-note
  '((t :inherit org-link :underline nil))
  "Face used for calendar dates with an existing daily note."
  :group 'org-slipbox)

;;; Bindings

;;;###autoload
(defvar-keymap org-slipbox-dailies-map
  :doc "Keymap for `org-slipbox-dailies' commands."
  "d" #'org-slipbox-dailies-goto-today
  "y" #'org-slipbox-dailies-goto-yesterday
  "t" #'org-slipbox-dailies-goto-tomorrow
  "n" #'org-slipbox-dailies-capture-today
  "f" #'org-slipbox-dailies-goto-next-note
  "b" #'org-slipbox-dailies-goto-previous-note
  "c" #'org-slipbox-dailies-goto-date
  "v" #'org-slipbox-dailies-capture-date
  "." #'org-slipbox-dailies-find-directory)

(defun org-slipbox-dailies--capture-template-key (template)
  "Return the selection key for daily TEMPLATE."
  (car template))

(defun org-slipbox-dailies--capture-template-description (template)
  "Return the user-facing description for daily TEMPLATE."
  (cadr template))

(defun org-slipbox-dailies--normalize-capture-heading (heading template)
  "Return the normalized daily capture HEADING for TEMPLATE.

When TEMPLATE does not consume title-derived placeholders, fall back to the
template description or key instead of forcing a meaningless prompt."
  (let ((heading (string-trim (or heading ""))))
    (cond
     ((not template)
      (when (string-empty-p heading)
        (user-error "Daily entry must not be empty"))
      heading)
     ((string-empty-p heading)
      (if (org-slipbox--capture-template-uses-title-p template)
          (user-error "Daily entry must not be empty")
        (or (org-slipbox-dailies--capture-template-description template)
            (org-slipbox-dailies--capture-template-key template)
            (user-error "Daily template must define a key or description"))))
     (t
      heading))))

(defun org-slipbox-dailies--read-capture-args (&optional keys)
  "Return interactive capture arguments as a list of HEADING and KEYS."
  (if org-slipbox-dailies-capture-templates
      (let* ((template (org-slipbox--read-capture-template
                        org-slipbox-dailies-capture-templates
                        keys))
             (heading (when (org-slipbox--capture-template-uses-title-p template)
                        (read-string "Daily entry: "))))
        (list heading (org-slipbox-dailies--capture-template-key template)))
    (list (read-string "Daily entry: ") keys)))

(define-minor-mode org-slipbox-dailies-calendar-mode
  "Mark visible calendar dates for existing daily notes.

This mode adds and removes calendar hooks explicitly instead of
installing them at load time."
  :global t
  :group 'org-slipbox
  (if org-slipbox-dailies-calendar-mode
      (progn
        (add-hook 'calendar-today-visible-hook
                  #'org-slipbox-dailies-calendar-mark-entries)
        (add-hook 'calendar-today-invisible-hook
                  #'org-slipbox-dailies-calendar-mark-entries))
    (remove-hook 'calendar-today-visible-hook
                 #'org-slipbox-dailies-calendar-mark-entries)
    (remove-hook 'calendar-today-invisible-hook
                 #'org-slipbox-dailies-calendar-mark-entries)))

;;;###autoload
(defun org-slipbox-dailies-capture-today (heading &optional keys)
  "Capture HEADING into today's daily note.
When KEYS is non-nil, use the matching daily capture template."
  (interactive (org-slipbox-dailies--read-capture-args))
  (org-slipbox-dailies--capture (current-time) heading keys))

;;;###autoload
(defun org-slipbox-dailies-goto-today ()
  "Visit today's daily note, creating it if needed."
  (interactive)
  (org-slipbox-dailies--goto (current-time)))

;;;###autoload
(defun org-slipbox-dailies-capture-tomorrow (n heading &optional keys)
  "Capture HEADING into the daily note N days in the future.
When KEYS is non-nil, use the matching daily capture template."
  (interactive
   (cons (prefix-numeric-value current-prefix-arg)
         (org-slipbox-dailies--read-capture-args)))
  (org-slipbox-dailies--capture (org-slipbox-dailies--offset-time n) heading keys))

;;;###autoload
(defun org-slipbox-dailies-goto-tomorrow (n)
  "Visit the daily note N days in the future, creating it if needed."
  (interactive "p")
  (org-slipbox-dailies--goto (org-slipbox-dailies--offset-time n)))

;;;###autoload
(defun org-slipbox-dailies-capture-yesterday (n heading &optional keys)
  "Capture HEADING into the daily note N days in the past.
When KEYS is non-nil, use the matching daily capture template."
  (interactive
   (cons (prefix-numeric-value current-prefix-arg)
         (org-slipbox-dailies--read-capture-args)))
  (org-slipbox-dailies--capture (org-slipbox-dailies--offset-time (- n)) heading keys))

;;;###autoload
(defun org-slipbox-dailies-goto-yesterday (n)
  "Visit the daily note N days in the past, creating it if needed."
  (interactive "p")
  (org-slipbox-dailies--goto (org-slipbox-dailies--offset-time (- n))))

;;;###autoload
(defun org-slipbox-dailies-capture-date (&optional prefer-future keys heading)
  "Capture a heading into a daily note selected with the calendar.
With prefix argument PREFER-FUTURE, `org-read-date' prefers future dates.
When KEYS is non-nil, use the matching daily capture template."
  (interactive
   (let ((args (org-slipbox-dailies--read-capture-args)))
     (list current-prefix-arg (cadr args) (car args))))
  (let ((time (org-slipbox-dailies--read-date "Capture to daily note: " prefer-future)))
    (org-slipbox-dailies--capture time heading keys)))

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

(defun org-slipbox-dailies-calendar--file-to-date (file)
  "Return FILE as a calendar date list, or nil when it is not parseable."
  (ignore-errors
    (let* ((parts (org-parse-time-string
                   (org-slipbox--file-name-stem file)))
           (day (nth 3 parts))
           (month (nth 4 parts))
           (year (nth 5 parts)))
      (and day month year (list month day year)))))

;;;###autoload
(defun org-slipbox-dailies-calendar-mark-entries ()
  "Mark visible calendar dates for existing daily notes.

Daily-note file names must remain parseable by `org-parse-time-string'."
  (interactive)
  (require 'calendar)
  (dolist (date (delq nil
                      (mapcar #'org-slipbox-dailies-calendar--file-to-date
                              (org-slipbox-dailies--list-files))))
    (when (calendar-date-is-visible-p date)
      (calendar-mark-visible-date date 'org-slipbox-dailies-calendar-note))))

(defun org-slipbox-dailies--goto (time)
  "Visit the daily note for TIME, creating it if needed."
  (let ((node (org-slipbox-dailies--ensure-note time)))
    (org-slipbox--visit-node node)
    (run-hooks 'org-slipbox-dailies-find-file-hook)
    node))

(defun org-slipbox-dailies--capture (time heading &optional keys)
  "Capture HEADING into the daily note for TIME.
When KEYS is non-nil, use the matching daily capture template."
  (let* ((template (and org-slipbox-dailies-capture-templates
                        (org-slipbox-dailies--capture-template
                         (org-slipbox--read-capture-template
                          org-slipbox-dailies-capture-templates
                          keys))))
         (heading (org-slipbox-dailies--normalize-capture-heading heading template))
         (finalize
          (lambda (captured _session)
            (org-slipbox--visit-node captured)
            (run-hooks 'org-slipbox-dailies-find-file-hook)))
         (node (if template
                   (org-slipbox--capture-node-at-time
                    heading
                    template
                    nil
                    time
                    nil
                    `(:default-finalize ,finalize))
                 (org-slipbox-rpc-append-heading
                  (org-slipbox-dailies--path time)
                  (org-slipbox-dailies--title time)
                  heading
                  org-slipbox-dailies-entry-level))))
    (when (plist-get node :file_path)
      (org-slipbox--visit-node node)
      (run-hooks 'org-slipbox-dailies-find-file-hook))
    node))

(defun org-slipbox-dailies--capture-template (template)
  "Return TEMPLATE adjusted for `org-slipbox-dailies-directory'."
  (when template
    (let ((prefix (if (org-slipbox--typed-capture-template-p template)
                      (seq-take template 4)
                    (seq-take template 2)))
          (options (org-slipbox-dailies--capture-template-options
                    (copy-tree (org-slipbox--capture-template-options template)))))
      (append prefix options))))

(defun org-slipbox-dailies--capture-template-options (options)
  "Return OPTIONS with file-based targets rooted in the dailies directory."
  (let ((rewritten (copy-tree options)))
    (when-let ((path (plist-get rewritten :path)))
      (setq rewritten
            (plist-put rewritten :path
                       (org-slipbox-dailies--capture-template-path path))))
    (when-let ((target (plist-get rewritten :target)))
      (setq rewritten
            (plist-put rewritten :target
                       (org-slipbox-dailies--capture-template-target target))))
    rewritten))

(defun org-slipbox-dailies--capture-template-target (target)
  "Return TARGET rewritten relative to `org-slipbox-dailies-directory'."
  (pcase target
    (`(file ,path)
     `(file ,(org-slipbox-dailies--capture-template-path path)))
    (`(file+head ,path ,head)
     `(file+head ,(org-slipbox-dailies--capture-template-path path) ,head))
    (`(file+olp ,path ,olp)
     `(file+olp ,(org-slipbox-dailies--capture-template-path path) ,olp))
    (`(file+head+olp ,path ,head ,olp)
     `(file+head+olp ,(org-slipbox-dailies--capture-template-path path) ,head ,olp))
    (`(file+datetree ,path . ,rest)
     `(file+datetree ,(org-slipbox-dailies--capture-template-path path) ,@rest))
    (_ target)))

(defun org-slipbox-dailies--capture-template-path (path)
  "Return PATH rooted in `org-slipbox-dailies-directory' when appropriate."
  (let ((directory (file-name-as-directory org-slipbox-dailies-directory)))
    (if (or (string-empty-p org-slipbox-dailies-directory)
            (file-name-absolute-p path)
            (string-prefix-p directory path))
        path
      (concat directory path))))

(defun org-slipbox-dailies--ensure-note (time)
  "Return the daily note node for TIME."
  (org-slipbox-rpc-ensure-file-node
   (org-slipbox-dailies--path time)
   (org-slipbox-dailies--title time)))

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
           (seq-filter (lambda (file)
                         (file-in-directory-p file directory))
                       (org-slipbox-list-files)))
          #'string-lessp)
       nil)
     extra-files)))

(defun org-slipbox-dailies--daily-note-p (&optional file)
  "Return non-nil when FILE is a daily note."
  (when-let* ((candidate (or file (org-slipbox-dailies--current-file)))
              (path (expand-file-name candidate))
              (directory (expand-file-name org-slipbox-dailies-directory org-slipbox-directory)))
    (and (org-slipbox-file-p path)
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
