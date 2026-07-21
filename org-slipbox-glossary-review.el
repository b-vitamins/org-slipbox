;;; org-slipbox-glossary-review.el --- Spaced-repetition review for glossary terms -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.16.0
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

;; A card-at-a-time spaced-repetition study loop for `org-slipbox' glossary
;; terms.  `org-slipbox-glossary-review' fetches the due queue through
;; `slipbox/glossaryDue' and presents one term at a time: the headword is shown
;; first, its definition is revealed on request, and a single SM-2 grade key
;; (`0'..`5') calls `slipbox/gradeTerm', reschedules the term, and advances to
;; the next card until the queue empties.

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'org-slipbox-glossary)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-glossary-review-limit 50
  "Maximum number of due glossary terms fetched for a review session."
  :type 'integer
  :group 'org-slipbox)

(defconst org-slipbox-glossary-review-buffer "*org-slipbox glossary review*"
  "Name of the glossary spaced-repetition review buffer.")

(defconst org-slipbox-glossary-review-grades
  '((?0 . "blackout")
    (?1 . "wrong; familiar")
    (?2 . "wrong; easy recall")
    (?3 . "correct; hard")
    (?4 . "correct; hesitant")
    (?5 . "correct; instant"))
  "SM-2 grade keys mapped to their recall-quality descriptions.")

(cl-defstruct org-slipbox-glossary-review-session
  "Explicit state for a glossary spaced-repetition review buffer."
  queue
  current
  revealed
  total
  graded
  last-outcome)

(defvar-local org-slipbox-glossary-review-session nil
  "Session state for the current glossary review buffer.")

(put 'org-slipbox-glossary-review-session 'permanent-local t)

(defvar org-slipbox-glossary-review-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "SPC") #'org-slipbox-glossary-review-reveal)
    (define-key map (kbd "TAB") #'org-slipbox-glossary-review-reveal)
    (define-key map (kbd "0") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "1") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "2") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "3") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "4") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "5") #'org-slipbox-glossary-review-grade)
    (define-key map (kbd "s") #'org-slipbox-glossary-review-skip)
    (define-key map (kbd "v") #'org-slipbox-glossary-review-visit)
    (define-key map (kbd "g") #'org-slipbox-glossary-review-refresh)
    (define-key map (kbd "q") #'quit-window)
    map)
  "Keymap for `org-slipbox-glossary-review-mode'.")

(define-derived-mode org-slipbox-glossary-review-mode special-mode
  "org-slipbox-review"
  "Major mode for the glossary spaced-repetition review buffer.")

;;;###autoload
(defun org-slipbox-glossary-review (&optional today)
  "Review glossary terms due on TODAY one card at a time.
TODAY is an ISO date string; when nil, the daemon uses its local date.
Displays a review buffer with single-key SM-2 grading."
  (interactive)
  (let ((buffer (get-buffer-create org-slipbox-glossary-review-buffer)))
    (with-current-buffer buffer
      (org-slipbox-glossary-review-mode)
      (setq-local org-slipbox-glossary-review-session
                  (org-slipbox-glossary-review--fetch-session today))
      (org-slipbox-glossary-review--render))
    (display-buffer buffer)
    buffer))

(defun org-slipbox-glossary-review-refresh ()
  "Re-fetch the due queue and restart the review session."
  (interactive)
  (org-slipbox-glossary-review--require-session)
  (setq-local org-slipbox-glossary-review-session
              (org-slipbox-glossary-review--fetch-session))
  (org-slipbox-glossary-review--render))

(defun org-slipbox-glossary-review-reveal ()
  "Reveal the definition of the current review card."
  (interactive)
  (let ((session (org-slipbox-glossary-review--require-session)))
    (unless (org-slipbox-glossary-review-session-current session)
      (user-error "No glossary term to reveal"))
    (setf (org-slipbox-glossary-review-session-revealed session) t)
    (org-slipbox-glossary-review--render)))

(defun org-slipbox-glossary-review-grade ()
  "Grade the current review card using the last key and advance the queue.
The invoking key, one of `0'..`5', is the SM-2 recall quality."
  (interactive)
  (let* ((session (org-slipbox-glossary-review--require-session))
         (node (org-slipbox-glossary-review-session-current session))
         (quality (- last-command-event ?0)))
    (unless node
      (user-error "No glossary term to grade"))
    (unless (org-slipbox-glossary-review-session-revealed session)
      (user-error "Reveal the definition before grading"))
    (let* ((node-key (plist-get node :node_key))
           (response (org-slipbox-rpc-grade-term node-key quality))
           (graded (or (plist-get response :term) node)))
      (setf (org-slipbox-glossary-review-session-last-outcome session)
            (list :title (plist-get graded :title)
                  :quality quality
                  :due (plist-get graded :sr_due)))
      (cl-incf (org-slipbox-glossary-review-session-graded session))
      (org-slipbox-glossary-review--advance session)
      (org-slipbox-glossary-review--render))))

(defun org-slipbox-glossary-review-skip ()
  "Skip the current review card without grading it."
  (interactive)
  (let ((session (org-slipbox-glossary-review--require-session)))
    (unless (org-slipbox-glossary-review-session-current session)
      (user-error "No glossary term to skip"))
    (setf (org-slipbox-glossary-review-session-last-outcome session)
          (list :title (plist-get
                         (org-slipbox-glossary-review-session-current session)
                         :title)
                :skipped t))
    (org-slipbox-glossary-review--advance session)
    (org-slipbox-glossary-review--render)))

(defun org-slipbox-glossary-review-visit ()
  "Visit the source file of the current review card in another window."
  (interactive)
  (let* ((session (org-slipbox-glossary-review--require-session))
         (node (org-slipbox-glossary-review-session-current session)))
    (unless node
      (user-error "No glossary term to visit"))
    (org-slipbox--visit-node node t)))

(defun org-slipbox-glossary-review--require-session ()
  "Return the current review session, or signal a user error."
  (unless (derived-mode-p 'org-slipbox-glossary-review-mode)
    (user-error "Not in an org-slipbox glossary review buffer"))
  (or org-slipbox-glossary-review-session
      (user-error "No active glossary review session")))

(defun org-slipbox-glossary-review--fetch-session (&optional today)
  "Return a fresh review session for the terms due on TODAY."
  (let* ((response (org-slipbox-rpc-glossary-due
                    today org-slipbox-glossary-review-limit))
         (terms (org-slipbox--plist-sequence (plist-get response :terms))))
    (make-org-slipbox-glossary-review-session
     :queue (cdr terms)
     :current (car terms)
     :revealed nil
     :total (length terms)
     :graded 0
     :last-outcome nil)))

(defun org-slipbox-glossary-review--advance (session)
  "Advance SESSION to the next due card."
  (setf (org-slipbox-glossary-review-session-current session)
        (car (org-slipbox-glossary-review-session-queue session))
        (org-slipbox-glossary-review-session-queue session)
        (cdr (org-slipbox-glossary-review-session-queue session))
        (org-slipbox-glossary-review-session-revealed session) nil))

(defun org-slipbox-glossary-review--render ()
  "Render the current review session into the current buffer."
  (let ((session org-slipbox-glossary-review-session)
        (inhibit-read-only t))
    (erase-buffer)
    (org-slipbox-glossary-review--insert-status session)
    (insert "\n")
    (if-let ((node (org-slipbox-glossary-review-session-current session)))
        (org-slipbox-glossary-review--insert-card session node)
      (org-slipbox-glossary-review--insert-done session))
    (goto-char (point-min))))

(defun org-slipbox-glossary-review--insert-status (session)
  "Insert the progress header line for SESSION."
  (let* ((total (org-slipbox-glossary-review-session-total session))
         (graded (org-slipbox-glossary-review-session-graded session))
         (remaining (org-slipbox-glossary-review--remaining session)))
    (insert (propertize
             (format "Glossary review: %d due, %d graded, %d remaining\n"
                     total graded remaining)
             'face 'bold))
    (when-let ((outcome (org-slipbox-glossary-review-session-last-outcome session)))
      (insert (org-slipbox-glossary-review--outcome-line outcome) "\n"))))

(defun org-slipbox-glossary-review--remaining (session)
  "Return the number of cards left in SESSION, including the current one."
  (+ (if (org-slipbox-glossary-review-session-current session) 1 0)
     (length (org-slipbox-glossary-review-session-queue session))))

(defun org-slipbox-glossary-review--outcome-line (outcome)
  "Return a summary line for the previous card OUTCOME."
  (cond
   ((plist-get outcome :skipped)
    (format "Skipped %s." (plist-get outcome :title)))
   (t
    (format "Graded %s: quality %d%s."
            (plist-get outcome :title)
            (plist-get outcome :quality)
            (if-let ((due (plist-get outcome :due)))
                (format ", next due %s" due)
              "")))))

(defun org-slipbox-glossary-review--insert-card (session node)
  "Insert the current review card for NODE in SESSION."
  (insert (propertize (or (plist-get node :title) "") 'face 'bold) "\n\n")
  (when-let ((synonyms (org-slipbox--plist-sequence (plist-get node :aliases))))
    (when synonyms
      (insert "Synonyms: " (string-join synonyms ", ") "\n\n")))
  (if (org-slipbox-glossary-review-session-revealed session)
      (let ((definition (org-slipbox-glossary--definition node)))
        (if (and definition (not (string-empty-p definition)))
            (insert definition "\n\n")
          (insert "(no definition)\n\n"))
        (insert (org-slipbox-glossary-review--grade-hint)))
    (insert "Definition hidden.\n\n")
    (insert "Press SPC to reveal.\n")))

(defun org-slipbox-glossary-review--grade-hint ()
  "Return the single-key SM-2 grade hint block."
  (concat "Grade recall:\n"
          (mapconcat
           (lambda (grade)
             (format "  %c  %s" (car grade) (cdr grade)))
           org-slipbox-glossary-review-grades
           "\n")
          "\n"))

(defun org-slipbox-glossary-review--insert-done (session)
  "Insert the completion message for a drained SESSION."
  (if (zerop (org-slipbox-glossary-review-session-total session))
      (insert "No glossary terms are due for review.\n")
    (insert (format "Review complete: graded %d of %d due terms.\n"
                    (org-slipbox-glossary-review-session-graded session)
                    (org-slipbox-glossary-review-session-total session)))))

(provide 'org-slipbox-glossary-review)

;;; org-slipbox-glossary-review.el ends here
