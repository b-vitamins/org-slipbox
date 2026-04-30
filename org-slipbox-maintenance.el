;;; org-slipbox-maintenance.el --- Diagnostics and maintenance for org-slipbox -*- lexical-binding: t; -*-

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

;; Explicit maintenance and diagnostics commands for `org-slipbox'.

;;; Code:

(require 'pp)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-discovery)
(require 'org-slipbox-files)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)
(require 'org-slipbox-sync)

(define-derived-mode org-slipbox-maintenance-mode special-mode "org-slipbox-Maintenance"
  "Major mode for org-slipbox diagnostics and maintenance buffers.")

;;;###autoload
(defun org-slipbox-sync ()
  "Synchronize the org-slipbox index with the current eligible files."
  (interactive)
  (let* ((response (org-slipbox-rpc-index))
         (files (plist-get response :files_indexed))
         (nodes (plist-get response :nodes_indexed))
         (links (plist-get response :links_indexed)))
    (message "Synced %s files, %s nodes, %s links" files nodes links)
    response))

;;;###autoload
(defun org-slipbox-rebuild ()
  "Rebuild the org-slipbox index from scratch."
  (interactive)
  (let ((database (expand-file-name org-slipbox-database-file)))
    (org-slipbox-rpc-reset)
    (org-slipbox--maintenance-delete-database-files database)
    (org-slipbox-sync)))

;;;###autoload
(defun org-slipbox-sync-current-file ()
  "Synchronize the current file into the org-slipbox index."
  (interactive)
  (let* ((buffer (or (buffer-base-buffer) (current-buffer)))
         (file (buffer-file-name buffer)))
    (unless file
      (user-error "Current buffer is not visiting a file"))
    (with-current-buffer buffer
      (when (buffer-modified-p)
        (save-buffer)))
    (let* ((response (org-slipbox-rpc-index-file file))
           (indexed-path (plist-get response :file_path)))
      (message "Synced %s" indexed-path)
      response)))

;;;###autoload
(defun org-slipbox-diagnose-node ()
  "Display diagnostics for the current node and file."
  (interactive)
  (let* ((status (org-slipbox-rpc-status))
         (indexed-files (org-slipbox--maintenance-indexed-files))
         (file (org-slipbox--maintenance-current-file))
         (diagnostics (and file
                           (org-slipbox--maintenance-file-diagnostics
                            file
                            indexed-files)))
         (node (org-slipbox-node-at-point)))
    (org-slipbox--maintenance-display-buffer
     "*org-slipbox diagnostics*"
     (lambda ()
       (org-slipbox--maintenance-insert-heading "Status")
       (org-slipbox--maintenance-insert-key-value "Version" (plist-get status :version))
       (org-slipbox--maintenance-insert-key-value "Root" (plist-get status :root))
       (org-slipbox--maintenance-insert-key-value "Database" (plist-get status :db))
       (org-slipbox--maintenance-insert-key-value "Autosync" (if org-slipbox-autosync-mode "enabled" "disabled"))
       (org-slipbox--maintenance-insert-key-value
        "Indexed counts"
        (format "%s files, %s nodes, %s links"
                (plist-get status :files_indexed)
                (plist-get status :nodes_indexed)
                (plist-get status :links_indexed)))
       (insert "\n")
       (org-slipbox--maintenance-insert-heading "Current File")
       (if diagnostics
           (org-slipbox--maintenance-insert-file-diagnostics diagnostics)
         (insert "No current file.\n\n"))
       (org-slipbox--maintenance-insert-heading "Current Node")
       (if node
           (insert (pp-to-string node))
         (insert "No canonical node at point.\n"))))))

;;;###autoload
(defun org-slipbox-diagnose-file (&optional file)
  "Display discovery and index diagnostics for FILE.
When FILE is nil, default to the current buffer file or prompt."
  (interactive)
  (let* ((file (or file
                   (org-slipbox--maintenance-current-file)
                   (read-file-name "Diagnose file: "
                                   (expand-file-name org-slipbox-directory))))
         (indexed-files (org-slipbox--maintenance-indexed-files))
         (diagnostics (org-slipbox--maintenance-file-diagnostics file indexed-files)))
    (org-slipbox--maintenance-display-buffer
     "*org-slipbox file diagnostics*"
     (lambda ()
       (org-slipbox--maintenance-insert-heading "File Diagnostics")
       (org-slipbox--maintenance-insert-file-diagnostics diagnostics)))))

;;;###autoload
(defun org-slipbox-list-files-report ()
  "Display eligible and indexed files for maintenance inspection."
  (interactive)
  (let* ((status (org-slipbox-rpc-status))
         (root (file-name-as-directory (expand-file-name org-slipbox-directory)))
         (eligible (mapcar (lambda (path)
                             (file-relative-name path root))
                           (org-slipbox-list-files root)))
         (indexed (org-slipbox--maintenance-indexed-files))
         (eligible-not-indexed (org-slipbox--maintenance-set-difference eligible indexed))
         (indexed-ineligible (org-slipbox--maintenance-set-difference indexed eligible)))
    (org-slipbox--maintenance-display-buffer
     "*org-slipbox files*"
     (lambda ()
       (org-slipbox--maintenance-insert-heading "Index Summary")
       (org-slipbox--maintenance-insert-key-value
        "Indexed counts"
        (format "%s files, %s nodes, %s links"
                (plist-get status :files_indexed)
                (plist-get status :nodes_indexed)
                (plist-get status :links_indexed)))
       (org-slipbox--maintenance-insert-key-value "Eligible files" (number-to-string (length eligible)))
       (org-slipbox--maintenance-insert-key-value "Indexed files" (number-to-string (length indexed)))
       (insert "\n")
       (org-slipbox--maintenance-insert-list "Eligible Files" eligible "No eligible files.")
       (org-slipbox--maintenance-insert-list "Indexed Files" indexed "No indexed files.")
       (org-slipbox--maintenance-insert-list
        "Eligible But Not Indexed"
        eligible-not-indexed
        "No eligible files are missing from the index.")
       (org-slipbox--maintenance-insert-list
        "Indexed But Not Eligible"
        indexed-ineligible
        "No indexed files are outside the current discovery policy.")))))

;;;###autoload
(defun org-slipbox-db-explore ()
  "Open the org-slipbox SQLite database in `sqlite-mode'."
  (interactive)
  (require 'sqlite-mode nil t)
  (if (fboundp 'sqlite-mode-open-file)
      (sqlite-mode-open-file (plist-get (org-slipbox-rpc-status) :db))
    (user-error "This command requires Emacs 29 sqlite-mode support")))

(defun org-slipbox--maintenance-display-buffer (name renderer)
  "Render NAME using RENDERER and display the resulting buffer."
  (with-current-buffer (get-buffer-create name)
    (let ((inhibit-read-only t))
      (erase-buffer)
      (org-slipbox-maintenance-mode)
      (funcall renderer)
      (goto-char (point-min)))
    (display-buffer (current-buffer))))

(defun org-slipbox--maintenance-current-file ()
  "Return the current file path, or nil when not visiting a file."
  (buffer-file-name (or (buffer-base-buffer) (current-buffer))))

(defun org-slipbox--maintenance-indexed-files ()
  "Return the indexed file paths as a list of relative strings."
  (org-slipbox--plist-sequence
   (plist-get (org-slipbox-rpc-indexed-files) :files)))

(defun org-slipbox--maintenance-file-diagnostics (file indexed-files)
  "Return a plist describing discovery and index state for FILE.
INDEXED-FILES is the current list of relative indexed file paths."
  (let* ((root (file-name-as-directory (expand-file-name org-slipbox-directory)))
         (expanded-file (expand-file-name file))
         (under-root (file-in-directory-p expanded-file root))
         (relative-path (and under-root (file-relative-name expanded-file root)))
         (base-extension (org-slipbox--file-base-extension expanded-file))
         (supported (and base-extension (org-slipbox--supported-file-p expanded-file)))
         (matched-excludes (and relative-path
                                (org-slipbox--maintenance-matched-exclude-regexps relative-path)))
         (eligible (and under-root supported (null matched-excludes)))
         (indexed (and relative-path (member relative-path indexed-files))))
    (list :file expanded-file
          :relative-path relative-path
          :exists (file-exists-p expanded-file)
          :under-root under-root
          :base-extension base-extension
          :supported supported
          :matched-excludes matched-excludes
          :eligible eligible
          :indexed indexed)))

(defun org-slipbox--maintenance-matched-exclude-regexps (relative-path)
  "Return exclusion regexps matching RELATIVE-PATH."
  (seq-filter
   (lambda (pattern)
     (string-match-p pattern relative-path))
   (org-slipbox-discovery-exclude-regexps)))

(defun org-slipbox--maintenance-insert-file-diagnostics (diagnostics)
  "Insert the file DIAGNOSTICS plist into the current buffer."
  (org-slipbox--maintenance-insert-key-value "File" (plist-get diagnostics :file))
  (org-slipbox--maintenance-insert-key-value
   "Exists"
   (if (plist-get diagnostics :exists) "yes" "no"))
  (org-slipbox--maintenance-insert-key-value
   "Under root"
   (if (plist-get diagnostics :under-root) "yes" "no"))
  (when-let ((relative-path (plist-get diagnostics :relative-path)))
    (org-slipbox--maintenance-insert-key-value "Relative path" relative-path))
  (org-slipbox--maintenance-insert-key-value
   "Base extension"
   (or (plist-get diagnostics :base-extension) "none"))
  (org-slipbox--maintenance-insert-key-value
   "Supported extension"
   (if (plist-get diagnostics :supported) "yes" "no"))
  (org-slipbox--maintenance-insert-key-value
   "Excluded by policy"
   (if (plist-get diagnostics :matched-excludes) "yes" "no"))
  (when-let ((patterns (plist-get diagnostics :matched-excludes)))
    (org-slipbox--maintenance-insert-key-value "Matched excludes" (string-join patterns ", ")))
  (org-slipbox--maintenance-insert-key-value
   "Eligible"
   (if (plist-get diagnostics :eligible) "yes" "no"))
  (org-slipbox--maintenance-insert-key-value
   "Indexed"
   (if (plist-get diagnostics :indexed) "yes" "no"))
  (insert "\n"))

(defun org-slipbox--maintenance-delete-database-files (database)
  "Delete DATABASE and known SQLite sidecar files when present."
  (dolist (file (list database
                      (concat database "-wal")
                      (concat database "-shm")))
    (when (file-exists-p file)
      (delete-file file))))

(defun org-slipbox--maintenance-set-difference (left right)
  "Return items in LEFT that are not present in RIGHT."
  (seq-remove (lambda (item)
                (member item right))
              left))

(defun org-slipbox--maintenance-insert-heading (title)
  "Insert a section heading TITLE."
  (insert title "\n")
  (insert (make-string (length title) ?=) "\n\n"))

(defun org-slipbox--maintenance-insert-key-value (label value)
  "Insert LABEL and VALUE on one line."
  (insert (propertize (format "%-16s " (concat label ":")) 'face 'bold)
          value
          "\n"))

(defun org-slipbox--maintenance-insert-list (title items empty-message)
  "Insert section TITLE containing ITEMS or EMPTY-MESSAGE."
  (insert title "\n")
  (insert (make-string (length title) ?-) "\n")
  (if items
      (dolist (item items)
        (insert item "\n"))
    (insert empty-message "\n"))
  (insert "\n"))

(provide 'org-slipbox-maintenance)

;;; org-slipbox-maintenance.el ends here
