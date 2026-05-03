;;; org-slipbox-capture-template.el --- Capture template helpers for org-slipbox -*- lexical-binding: t; -*-

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

;; Capture template configuration, expansion, and target-preparation helpers.

;;; Code:

(require 'org)
(require 'org-capture)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-node)

(defconst org-slipbox--capture-types '(plain entry item checkitem table-line)
  "Org-roam-style capture content types supported by org-slipbox.")

(defcustom org-slipbox-capture-templates
  '(("d" "default" :path "${slug}.org" :title "${title}"))
  "Capture templates for `org-slipbox-capture'.
Each template is a list of the form
\(KEY DESCRIPTION [:path STRING] [:title STRING] [:target SPEC]\)
or
\(KEY DESCRIPTION TYPE TEMPLATE . OPTIONS\).

When present, SPEC may be one of:
- (file PATH)
- (file+head PATH HEAD)
- (file+olp PATH (OUTLINE ...))
- (file+head+olp PATH HEAD (OUTLINE ...))
- (file+datetree PATH [TREE-TYPE])
- (node TITLE-OR-ALIAS-OR-ID)

TYPE may be one of `plain', `entry', `item', `checkitem', or
`table-line'. Typed templates follow the org-roam capture model:
`:target' selects an existing or created location and TYPE/TEMPLATE
describe the content inserted there. The older shorthand template
syntax is preserved for compatibility.

Typed `table-line' templates may also set `:table-line-pos' to place
the inserted row relative to table separators.

In addition to target and content options, typed templates may carry
the lifecycle keys `:finalize', `:jump-to-captured',
`:immediate-finish', `:prepare-finalize', `:before-finalize', and
`:after-finalize', `:kill-buffer', `:no-save', `:unnarrowed',
`:clock-in', `:clock-keep', and `:clock-resume'. Every capture starts
in a transient draft buffer unless `:immediate-finish' commits the
prepared draft directly, and all target writes still go through the
Rust RPC layer."
  :type 'sexp
  :group 'org-slipbox)

(defcustom org-slipbox-capture-ref-templates
  '(("r" "ref"
     :target (file+head
              "${slug}.org"
              "#+title: ${title}\n\n- source :: ${ref}\n\n${body}")
     :title "${title}"))
  "Capture templates used by ref-oriented capture workflows.
These templates use the same syntax as `org-slipbox-capture-templates'
and may interpolate `${ref}', `${body}', `${annotation}', and `${link}'."
  :type 'sexp
  :group 'org-slipbox)

(defconst org-slipbox--capture-unsupported-lifecycle-keys
  nil
  "Template lifecycle keys reserved for later capture-parity work.")

(defconst org-slipbox--capture-phase-keys
  '(:prepare-finalize :before-finalize :after-finalize)
  "Lifecycle hook keys supported by org-slipbox capture templates.")

(defconst org-slipbox--capture-unsupported-target-keys
  '(:exact-position :insert-here)
  "Target-preparation keys that currently error instead of being ignored.")

(defun org-slipbox--default-capture-template ()
  "Return the default capture template."
  (car org-slipbox-capture-templates))

(defun org-slipbox--typed-capture-template-p (template)
  "Return non-nil when TEMPLATE uses the explicit typed syntax."
  (memq (nth 2 template) org-slipbox--capture-types))

(defun org-slipbox--capture-template-options (template)
  "Return the plist options for TEMPLATE."
  (if (org-slipbox--typed-capture-template-p template)
      (nthcdr 4 template)
    (nthcdr 2 template)))

(defconst org-slipbox--capture-title-placeholder-regexp
  "\\${\\(?:title\\|slug\\)\\(?:=[^}]*\\)?}"
  "Regexp matching capture placeholders that depend on the note title.")

(defun org-slipbox--capture-template-type (template)
  "Return the content type for TEMPLATE."
  (if (org-slipbox--typed-capture-template-p template)
      (nth 2 template)
    'entry))

(defun org-slipbox--capture-string-uses-title-p (string)
  "Return non-nil when STRING references title-derived placeholders."
  (and (stringp string)
       (string-match-p org-slipbox--capture-title-placeholder-regexp string)))

(defun org-slipbox--capture-sequence-uses-title-p (sequence)
  "Return non-nil when any string in SEQUENCE references title placeholders."
  (seq-some #'org-slipbox--capture-string-uses-title-p sequence))

(defun org-slipbox--capture-template-source-uses-title-p (source)
  "Return non-nil when SOURCE depends on title-derived placeholders."
  (pcase source
    (`(file ,path)
     (or (org-slipbox--capture-string-uses-title-p path)
         (and (stringp path)
              (file-readable-p path)
              (with-temp-buffer
                (insert-file-contents path)
                (org-slipbox--capture-string-uses-title-p (buffer-string))))
         (not (and (stringp path)
                   (file-readable-p path)))))
    ((pred functionp) t)
    ((pred stringp) (org-slipbox--capture-string-uses-title-p source))
    ((or `nil `()) nil)
    (_ t)))

(defun org-slipbox--capture-target-uses-title-p (target)
  "Return non-nil when TARGET depends on title-derived placeholders."
  (pcase target
    (`(file ,path)
     (org-slipbox--capture-string-uses-title-p path))
    (`(file+head ,path ,head)
     (or (org-slipbox--capture-string-uses-title-p path)
         (org-slipbox--capture-string-uses-title-p head)))
    (`(file+olp ,path ,olp)
     (or (org-slipbox--capture-string-uses-title-p path)
         (org-slipbox--capture-sequence-uses-title-p olp)))
    (`(file+head+olp ,path ,head ,olp)
     (or (org-slipbox--capture-string-uses-title-p path)
         (org-slipbox--capture-string-uses-title-p head)
         (org-slipbox--capture-sequence-uses-title-p olp)))
    (`(file+datetree ,path . ,_)
     (org-slipbox--capture-string-uses-title-p path))
    (`(node ,query)
     (org-slipbox--capture-string-uses-title-p query))
    (_ nil)))

(defun org-slipbox--capture-template-uses-title-p (template)
  "Return non-nil when TEMPLATE requires a title-derived value."
  (let* ((options (org-slipbox--capture-template-options template))
         (typed (org-slipbox--typed-capture-template-p template))
         (capture-type (org-slipbox--capture-template-type template))
         (source (and typed (nth 3 template))))
    (or (org-slipbox--capture-string-uses-title-p (plist-get options :title))
        (org-slipbox--capture-string-uses-title-p (plist-get options :path))
        (org-slipbox--capture-target-uses-title-p (plist-get options :target))
        (if source
            (org-slipbox--capture-template-source-uses-title-p source)
          (if typed
              (org-slipbox--capture-string-uses-title-p
               (org-slipbox--default-capture-body-template capture-type))
            (not (plist-member options :title)))))))

(defun org-slipbox--capture-template-time (time options)
  "Return the effective capture TIME for OPTIONS."
  (or time
      (and (plist-get options :time-prompt)
           (org-read-date nil t nil "Capture date: "))
      (current-time)))

(defun org-slipbox--capture-validate-template-options
    (template-options capture-type)
  "Signal a clear error for unsupported keys in TEMPLATE-OPTIONS.
CAPTURE-TYPE is the effective type for the selected template."
  (dolist (key org-slipbox--capture-unsupported-lifecycle-keys)
    (when (plist-member template-options key)
      (user-error "Capture option %S is not implemented yet" key)))
  (dolist (key org-slipbox--capture-unsupported-target-keys)
    (when (plist-member template-options key)
      (user-error "Capture option %S is not implemented yet" key)))
  (dolist (key org-slipbox--capture-phase-keys)
    (when (plist-member template-options key)
      (org-slipbox--capture-validate-phase-functions
       key
       (plist-get template-options key))))
  (when (plist-member template-options :finalize)
    (org-slipbox--capture-validate-finalize
     (plist-get template-options :finalize)))
  (when (and (plist-member template-options :table-line-pos)
             (not (eq capture-type 'table-line)))
    (user-error "Capture option :table-line-pos requires `table-line' capture")))

(defun org-slipbox--capture-validate-phase-functions (key value)
  "Signal a user error when VALUE for lifecycle KEY is invalid."
  (unless (or (functionp value)
              (and (listp value)
                   (not (functionp value))
                   (seq-every-p #'functionp value)))
    (user-error "Capture option %S must be a function or list of functions" key)))

(defun org-slipbox--capture-validate-finalize (finalize)
  "Signal a user error when FINALIZE is invalid."
  (cond
   ((symbolp finalize)
    (unless (functionp (intern-soft
                        (format "org-slipbox--capture-finalize-%s" finalize)))
      (user-error "Unsupported capture finalize action %S" finalize)))
   ((functionp finalize) nil)
   (t
    (user-error "Unsupported capture finalize action %S" finalize))))

(defun org-slipbox--capture-shorthand-type (target)
  "Return the generic RPC capture type for shorthand TARGET."
  (if (or (plist-get target :outline_path)
          (eq (plist-get target :kind) 'file+olp)
          (eq (plist-get target :kind) 'node))
      'entry
    'plain))

(defun org-slipbox--capture-target-params (target)
  "Return generic capture RPC params for TARGET."
  (pcase (plist-get target :kind)
    ((or 'file 'file+olp)
     (append
      (when-let ((file-path (plist-get target :file_path)))
        (list :file_path file-path))
      (when-let ((head (plist-get target :head)))
        (list :head head))
      (when-let ((outline-path (plist-get target :outline_path)))
        (list :outline_path outline-path))))
    ('node
     (list :node_key (plist-get target :node_key)))
    (_
     (user-error "Unsupported capture target"))))

(defun org-slipbox--expand-capture-target (template-options title time &optional variables)
  "Expand TEMPLATE-OPTIONS for TITLE at TIME into a capture target plist."
  (let ((target (plist-get template-options :target)))
    (cond
     (target
      (pcase target
        (`(file ,path)
         `(:kind file
           :file_path ,(org-slipbox--expand-capture-template path title time variables)))
        (`(file+head ,path ,head)
         `(:kind file
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :head ,(org-slipbox--expand-capture-template head title time variables)))
        (`(file+olp ,path ,olp)
         `(:kind file+olp
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :outline_path ,(mapcar (lambda (heading)
                                    (org-slipbox--expand-capture-template
                                     heading title time variables))
                                  olp)))
        (`(file+head+olp ,path ,head ,olp)
         `(:kind file+olp
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :head ,(org-slipbox--expand-capture-template head title time variables)
           :outline_path ,(mapcar (lambda (heading)
                                    (org-slipbox--expand-capture-template
                                     heading title time variables))
                                  olp)))
        (`(file+datetree ,path . ,rest)
         `(:kind file
           :file_path ,(org-slipbox--expand-capture-template path title time variables)
           :outline_path
           ,(org-slipbox--capture-datetree-outline-path
             time
             (or (car rest)
                 (plist-get template-options :tree-type)
                 'day))))
        (`(node ,query)
         (let ((node (org-slipbox--resolve-capture-target-node
                      (org-slipbox--expand-capture-template query title time variables))))
           `(:kind node
             :node_key ,(plist-get node :node_key)
             :file_path ,(plist-get node :file_path))))
        (_
         (user-error "Unsupported capture target %S" target))))
     ((plist-get template-options :path)
      `(:kind file
        :file_path ,(org-slipbox--expand-capture-template
                     (plist-get template-options :path)
                     title
                     time
                     variables)))
     (t
      '(:kind file)))))

(defun org-slipbox--read-capture-template (templates &optional keys)
  "Return a capture template from TEMPLATES, optionally selected by KEYS."
  (cond
   (keys
    (or (seq-find (lambda (template)
                    (equal (car template) keys))
                  templates)
        (user-error "No capture template with key %s" keys)))
   ((null templates) nil)
   ((= (length templates) 1)
    (car templates))
   (t
    (let* ((choices (mapcar (lambda (template)
                              (cons (format "%s %s" (car template) (cadr template))
                                    template))
                            templates))
           (selection (completing-read "Template: " choices nil t)))
      (cdr (assoc selection choices))))))

(defun org-slipbox--expand-capture-template (template title time &optional variables)
  "Expand TEMPLATE for TITLE using TIME and VARIABLES."
  (when template
    (org-slipbox--render-capture-string template title time variables)))

(defun org-slipbox--render-capture-body (template capture-type title time &optional variables)
  "Render TEMPLATE for CAPTURE-TYPE with TITLE at TIME using VARIABLES."
  (let* ((template (org-slipbox--capture-template-source template title time variables))
         (template (or template
                       (org-slipbox--default-capture-body-template capture-type)))
         (rendered (org-slipbox--render-capture-string template title time variables)))
    (replace-regexp-in-string "%\\?" "" rendered t t)))

(defun org-slipbox--capture-template-source (template title time &optional variables)
  "Return the source string for TEMPLATE with TITLE, TIME, and VARIABLES."
  (pcase template
    (`(file ,path)
     (with-temp-buffer
       (insert-file-contents
        (org-slipbox--render-capture-string path title time variables))
       (buffer-string)))
    ((pred functionp)
     (funcall template))
    ((pred stringp)
     template)
    ((or `nil `())
     nil)
    (_
     (user-error "Unsupported capture template source %S" template))))

(defun org-slipbox--default-capture-body-template (capture-type)
  "Return the default body template for CAPTURE-TYPE."
  (pcase capture-type
    ('entry "* ${title}")
    ('item "- ${title}")
    ('checkitem "- [ ] ${title}")
    ('table-line "| ${title} |")
    (_ "")))

(defun org-slipbox--render-capture-string (template title time &optional variables)
  "Expand TEMPLATE for TITLE using TIME and VARIABLES."
  (let* ((context (org-slipbox--capture-template-context title variables))
         (formatted-state (org-slipbox--format-capture-template template context))
         (formatted (car formatted-state))
         (context (cdr formatted-state))
         (org-store-link-plist
          (org-slipbox--capture-store-link-plist context))
         (org-capture-plist
          (list :default-time time
                :buffer (current-buffer)
                :annotation (plist-get context :annotation)
                :initial (plist-get context :body))))
    (replace-regexp-in-string
     "[\n]*\\'"
     ""
     (org-capture-fill-template formatted))))

(defun org-slipbox--capture-template-context (title variables)
  "Return template expansion variables for TITLE merged with VARIABLES."
  (let ((context (list :title title
                       :slug (org-slipbox--slugify title)
                       :ref ""
                       :body ""
                       :annotation ""
                       :link "")))
    (while variables
      (setq context (plist-put context (car variables) (cadr variables))
            variables (cddr variables)))
    context))

(defun org-slipbox--format-capture-template (template context)
  "Expand `${...}' placeholders in TEMPLATE using CONTEXT."
  (let ((saved-match-data (match-data))
        (state (copy-sequence context)))
    (unwind-protect
        (cons
         (replace-regexp-in-string
          "\\${\\([^}]+\\)}"
          (lambda (match)
            (let* ((placeholder (match-string 1 match))
                   (placeholder-match-data (match-data))
                   key
                   default)
              (when (string-match "\\(.+\\)=\\(.*\\)" placeholder)
                (setq default (match-string 2 placeholder)
                      placeholder (match-string 1 placeholder)))
              (setq key (intern (concat ":" placeholder)))
              (unwind-protect
                  (let ((value
                         (cond
                          ((plist-member state key)
                           (or (plist-get state key) ""))
                          (t
                           (let* ((name (string-remove-prefix ":" (symbol-name key)))
                                  (input (read-from-minibuffer
                                          (format "%s: " name)
                                          default)))
                             (setq state (plist-put state key input))
                             input)))))
                    (if value
                        (format "%s" value)
                      match))
                (set-match-data placeholder-match-data))))
          template
          t
          t)
         state)
      (set-match-data saved-match-data))))

(defun org-slipbox--capture-store-link-plist (context)
  "Build an `org-store-link-plist' value from CONTEXT."
  (let (plist)
    (dolist (key '(:annotation :link :ref :body :title))
      (when (plist-member context key)
        (setq plist (plist-put plist key (plist-get context key)))))
    (plist-put plist :initial (plist-get context :body))))

(defun org-slipbox--capture-datetree-outline-path (time &optional tree-type)
  "Return a datetree outline path for TIME and TREE-TYPE."
  (pcase (or tree-type 'day)
    ('month
     (list (format-time-string "%Y" time)
           (format-time-string "%Y-%m %B" time)))
    ('week
     (list (format-time-string "%G" time)
           (format-time-string "%G-W%V" time)
           (format-time-string "%Y-%m-%d %A" time)))
    (_
     (list (format-time-string "%Y" time)
           (format-time-string "%Y-%m %B" time)
           (format-time-string "%Y-%m-%d %A" time)))))

(defun org-slipbox--capture-template-empty-lines (options)
  "Return normalized blank-line settings from OPTIONS."
  (let* ((common (max 0 (or (plist-get options :empty-lines) 0)))
         (before (or (plist-get options :empty-lines-before) common))
         (after (or (plist-get options :empty-lines-after) common)))
    (list :before before :after after)))

(defun org-slipbox--resolve-capture-target-node (query)
  "Resolve QUERY to an existing indexed node."
  (or (org-slipbox-node-from-id query)
      (org-slipbox-node-from-title-or-alias query t)
      (user-error "No existing node matches %s" query)))

(defun org-slipbox--slugify (title)
  "Convert TITLE into a stable file-name slug."
  (let ((result "")
        (previous-dash nil))
    (dolist (character (string-to-list title))
      (let ((normalized (downcase character)))
        (cond
         ((or (and (<= ?a normalized) (<= normalized ?z))
              (and (<= ?0 normalized) (<= normalized ?9)))
          (setq result (concat result (string normalized))
                previous-dash nil))
         ((not previous-dash)
          (setq result (concat result "-")
                previous-dash t)))))
    (let ((trimmed (string-trim result "-+" "-+")))
      (if (string-empty-p trimmed)
          "note"
        trimmed))))

(provide 'org-slipbox-capture-template)

;;; org-slipbox-capture-template.el ends here
