;;; org-slipbox-capture.el --- Capture commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Capture commands and template expansion helpers for `org-slipbox'.

;;; Code:

(require 'cl-lib)
(require 'org)
(require 'org-capture)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defconst org-slipbox--capture-types '(plain entry item checkitem table-line)
  "Org-roam-style capture content types supported by org-slipbox.")

(defvar org-slipbox-post-node-insert-hook nil
  "Hook run after `org-slipbox' inserts a new `id:' link.
Hook functions receive two arguments: the inserted ID and description.")

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

In addition to target and content options, typed templates may carry
the lifecycle keys `:finalize' and `:jump-to-captured'. Compatibility
keys from `org-capture', including `:immediate-finish', `:kill-buffer',
`:no-save', `:unnarrowed', `:clock-in', `:clock-resume', and
`:clock-keep', are accepted so org-roam-style templates can be reused,
but org-slipbox does not create a transient capture buffer, so the
runtime effect is driven by the post-success finalize action."
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

(defun org-slipbox--capture-node (title &optional template refs variables session)
  "Capture a new node with TITLE using TEMPLATE, REFS, VARIABLES, and SESSION."
  (org-slipbox--capture-node-at-time title template refs nil variables session))

(defun org-slipbox--capture-node-at-time
    (title &optional template refs time variables session)
  "Capture a new node with TITLE using TEMPLATE, REFS, TIME, VARIABLES, and SESSION."
  (let* ((template (or template (org-slipbox--default-capture-template)))
         (template-options (org-slipbox--capture-template-options template))
         (session (copy-sequence session))
         (time (org-slipbox--capture-template-time time template-options)))
    (org-slipbox--capture-run-lifecycle
     (if (org-slipbox--typed-capture-template-p template)
         (org-slipbox--capture-typed-template title template refs time variables)
       (org-slipbox--capture-shorthand-template title template refs time variables))
     template-options
     session)))

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

(defun org-slipbox--capture-template-type (template)
  "Return the content type for TEMPLATE."
  (if (org-slipbox--typed-capture-template-p template)
      (nth 2 template)
    'entry))

(defun org-slipbox--capture-template-time (time options)
  "Return the effective capture TIME for OPTIONS."
  (or time
      (and (plist-get options :time-prompt)
           (org-read-date nil t nil "Capture date: "))
      (current-time)))

(defun org-slipbox--capture-shorthand-template (title template refs time variables)
  "Capture TITLE with shorthand TEMPLATE using REFS, TIME, and VARIABLES."
  (let* ((template-options (org-slipbox--capture-template-options template))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             time
                             variables)
                            title))
         (target (org-slipbox--expand-capture-target
                  template-options
                  title
                  time
                  variables)))
    (pcase (plist-get target :kind)
      ('file
       (let ((params (if-let ((file-path (plist-get target :file_path)))
                         `(:title ,capture-title :file_path ,file-path)
                       `(:title ,capture-title))))
         (when-let ((head (plist-get target :head)))
           (setq params (plist-put params :head head)))
         (when refs
           (setq params (plist-put params :refs refs)))
         (org-slipbox-rpc-capture-node params)))
      ('file+olp
       (org-slipbox-rpc-append-heading-at-outline-path
        (append
         (list :file_path (plist-get target :file_path)
               :heading capture-title
               :outline_path (plist-get target :outline_path))
         (when-let ((head (plist-get target :head)))
           (list :head head)))))
      (_
       (user-error "Unsupported capture target")))))

(defun org-slipbox--capture-typed-template (title template refs time variables)
  "Capture TITLE using explicit typed TEMPLATE, REFS, TIME, and VARIABLES."
  (let* ((template-options (org-slipbox--capture-template-options template))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             time
                             variables)
                            title))
         (target (org-slipbox--expand-capture-target
                  template-options
                  capture-title
                  time
                  variables))
         (capture-type (org-slipbox--capture-template-type template))
         (content (org-slipbox--render-capture-body
                   (nth 3 template)
                   capture-type
                   capture-title
                   time
                   variables))
         (empty-lines (org-slipbox--capture-template-empty-lines template-options))
         (params (append
                  (list :title capture-title
                        :capture_type (symbol-name capture-type)
                        :content content
                        :prepend (and (plist-get template-options :prepend) t)
                        :empty_lines_before (plist-get empty-lines :before)
                        :empty_lines_after (plist-get empty-lines :after))
                  (when refs
                    (list :refs refs))
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
                     (user-error "Unsupported capture target"))))))
    (org-slipbox-rpc-capture-template params)))

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
           `(:kind node :node_key ,(plist-get node :node_key))))
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

(defun org-slipbox--capture-run-lifecycle (node template-options session)
  "Apply template and caller lifecycle settings for NODE."
  (let ((finalize (org-slipbox--capture-resolve-finalize template-options session)))
    (when finalize
      (org-slipbox--capture-call-finalizer finalize node session))
    node))

(defun org-slipbox--capture-resolve-finalize (template-options session)
  "Resolve the effective finalize action for TEMPLATE-OPTIONS and SESSION."
  (or (plist-get session :finalize)
      (plist-get template-options :finalize)
      (and (plist-get template-options :jump-to-captured) 'find-file)
      (plist-get session :default-finalize)))

(defun org-slipbox--capture-call-finalizer (finalize node session)
  "Run FINALIZE for NODE with SESSION."
  (cond
   ((symbolp finalize)
    (let ((function (intern-soft
                     (format "org-slipbox--capture-finalize-%s" finalize))))
      (unless (functionp function)
        (user-error "Unsupported capture finalize action %S" finalize))
      (funcall function node session)))
   ((functionp finalize)
    (funcall finalize node session))
   (t
    (user-error "Unsupported capture finalize action %S" finalize))))

(defun org-slipbox--capture-finalize-find-file (node _session)
  "Visit NODE after capture."
  (org-slipbox--visit-node node))

(defun org-slipbox--capture-finalize-insert-link (node session)
  "Insert a link to NODE using SESSION caller context."
  (let ((marker (plist-get session :call-location)))
    (unless marker
      (user-error "No caller location available for insert-link finalize"))
    (let ((buffer (marker-buffer marker)))
      (unless (buffer-live-p buffer)
        (user-error "The caller buffer for insert-link is no longer live"))
      (with-current-buffer buffer
        (goto-char marker)
        (when-let ((region (plist-get session :region)))
          (delete-region (car region) (cdr region))
          (goto-char (car region))
          (set-marker (car region) nil)
          (set-marker (cdr region) nil))
        (let* ((node-with-id (org-slipbox--ensure-node-id node))
               (description (or (plist-get session :link-description)
                                (org-slipbox-node-formatted node-with-id))))
          (insert (format "[[id:%s][%s]]"
                          (plist-get node-with-id :explicit_id)
                          description))
          (run-hook-with-args 'org-slipbox-post-node-insert-hook
                              (plist-get node-with-id :explicit_id)
                              description))
        (set-marker marker nil)))))

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

(provide 'org-slipbox-capture)

;;; org-slipbox-capture.el ends here
