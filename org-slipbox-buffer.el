;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.3.0
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

;; Context buffer commands for `org-slipbox'.

;;; Code:

(require 'button)
(require 'cl-lib)
(require 'org)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-files)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defvar org-slipbox-buffer "*org-slipbox*"
  "Name of the persistent org-slipbox context buffer.")

(defcustom org-slipbox-buffer-expensive-sections 'dedicated
  "When expensive discovery sections should be rendered.
`dedicated' renders them only in dedicated org-slipbox buffers,
`always' renders them everywhere, and nil disables them."
  :type '(choice
          (const :tag "Never" nil)
          (const :tag "Dedicated Buffers" dedicated)
          (const :tag "Always" always))
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-sections
  (list #'org-slipbox-buffer-node-section
        #'org-slipbox-buffer-refs-section
        #'org-slipbox-buffer-backlinks-section
        #'org-slipbox-buffer-forward-links-section
        #'org-slipbox-buffer-reflinks-section
        #'org-slipbox-buffer-unlinked-references-section)
  "Section specifications rendered by `org-slipbox-buffer-render-contents'.

Each item is either a function called with the current node, or a
list whose car is a function and whose remaining items are passed as
additional arguments. For example:

  (org-slipbox-buffer-backlinks-section :unique t
                                        :section-heading \"Unique Backlinks\")"
  :type `(repeat (choice (symbol :tag "Function")
                         (list :tag "Function with arguments"
                               (symbol :tag "Function")
                               (repeat :tag "Arguments" :inline t (sexp :tag "Arg")))))
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-postrender-functions nil
  "Functions run after an org-slipbox buffer has been rendered.
Each function is called with the rendered buffer as current."
  :type 'hook
  :group 'org-slipbox)

(defcustom org-slipbox-buffer-section-filter-function nil
  "Optional predicate controlling whether a section should render.
When non-nil, this function is called with SECTION-SPEC and NODE.
Return non-nil to render the section, or nil to skip it."
  :type '(choice (const :tag "None" nil) function)
  :group 'org-slipbox)

(cl-defstruct org-slipbox-buffer-session
  "Explicit session state for an org-slipbox context buffer."
  kind
  current-node
  root-node
  active-lens
  history
  lens-cache)

(defvar-local org-slipbox-buffer-session nil
  "Explicit session state for the current org-slipbox context buffer.")

(put 'org-slipbox-buffer-session 'permanent-local t)

(define-derived-mode org-slipbox-buffer-mode special-mode "org-slipbox"
  "Major mode for org-slipbox context buffers.")

(define-minor-mode org-slipbox-buffer-persistent-mode
  "Keep the persistent org-slipbox context buffer synchronized with point."
  :global t
  :group 'org-slipbox
  (if org-slipbox-buffer-persistent-mode
      (add-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)
    (remove-hook 'post-command-hook #'org-slipbox-buffer--redisplay-h)))

;;;###autoload
(defun org-slipbox-buffer-refresh ()
  "Refresh the current org-slipbox context buffer."
  (interactive)
  (unless (derived-mode-p 'org-slipbox-buffer-mode)
    (user-error "Not in an org-slipbox buffer"))
  (unless (org-slipbox-buffer--session-node)
    (user-error "No org-slipbox node to refresh"))
  (org-slipbox-buffer--clear-lens-cache)
  (org-slipbox-buffer-render-contents))

;;;###autoload
(defun org-slipbox-buffer-display-dedicated (node)
  "Display a dedicated org-slipbox buffer for NODE."
  (interactive (list (org-slipbox-buffer--read-node-for-display)))
  (let ((buffer (get-buffer-create (org-slipbox-buffer--dedicated-name node))))
    (with-current-buffer buffer
      (setq-local org-slipbox-buffer-session
                  (org-slipbox-buffer--make-dedicated-session node))
      (org-slipbox-buffer-render-contents))
    (display-buffer buffer)))

;;;###autoload
(defun org-slipbox-buffer-toggle ()
  "Toggle display of the persistent org-slipbox context buffer."
  (interactive)
  (if (get-buffer-window org-slipbox-buffer 'visible)
      (progn
        (quit-window nil (get-buffer-window org-slipbox-buffer))
        (org-slipbox-buffer-persistent-mode -1))
    (display-buffer (get-buffer-create org-slipbox-buffer))
    (org-slipbox-buffer-persistent-redisplay)
    (org-slipbox-buffer-persistent-mode 1)))

(defun org-slipbox-buffer-persistent-redisplay ()
  "Refresh the persistent org-slipbox context buffer from point."
  (when-let ((node (org-slipbox-node-at-point)))
    (with-current-buffer (get-buffer-create org-slipbox-buffer)
      (let ((session (or org-slipbox-buffer-session
                         (org-slipbox-buffer--make-persistent-session))))
        (unless (equal node (org-slipbox-buffer-session-current-node session))
          (setf (org-slipbox-buffer-session-current-node session) node
                (org-slipbox-buffer-session-root-node session) node
                (org-slipbox-buffer-session-lens-cache session) nil)
          (setq-local org-slipbox-buffer-session session)
          (org-slipbox-buffer-render-contents)
          (add-hook 'kill-buffer-hook #'org-slipbox-buffer--persistent-cleanup-h nil t))))))

(defun org-slipbox-buffer-render-contents ()
  "Render the current org-slipbox context buffer."
  (let* ((node (org-slipbox-buffer--session-node))
         (inhibit-read-only t))
    (erase-buffer)
    (org-slipbox-buffer-mode)
    (setq-local header-line-format
                (when node
                  (concat (propertize " " 'display '(space :align-to 0))
                          (plist-get node :title))))
    (when node
      (org-slipbox-buffer--render-sections node))
    (run-hooks 'org-slipbox-buffer-postrender-functions)
    (goto-char (point-min))))

(defun org-slipbox-buffer--render-sections (node)
  "Render configured sections for NODE."
  (dolist (section org-slipbox-buffer-sections)
    (when (org-slipbox-buffer--section-allowed-p section node)
      (org-slipbox-buffer--render-section section node))))

(defun org-slipbox-buffer--section-allowed-p (section node)
  "Return non-nil when SECTION should render for NODE."
  (or (null org-slipbox-buffer-section-filter-function)
      (funcall org-slipbox-buffer-section-filter-function section node)))

(defun org-slipbox-buffer--render-section (section node)
  "Render SECTION for NODE."
  (pcase section
    ((pred functionp)
     (funcall section node))
    (`(,fn . ,args)
     (unless (functionp fn)
       (user-error "Invalid org-slipbox buffer section function: %S" fn))
     (apply fn node args))
    (_
     (user-error "Invalid `org-slipbox-buffer-sections' specification: %S" section))))

(defun org-slipbox-buffer--redisplay-h ()
  "Keep the persistent org-slipbox context buffer in sync with point."
  (when (and (get-buffer-window org-slipbox-buffer 'visible)
             (not (buffer-modified-p (or (buffer-base-buffer) (current-buffer)))))
    (org-slipbox-buffer-persistent-redisplay)))

(defun org-slipbox-buffer--persistent-cleanup-h ()
  "Clean up persistent buffer global state."
  (when (string= (buffer-name) org-slipbox-buffer)
    (org-slipbox-buffer-persistent-mode -1)))

(defun org-slipbox-buffer--make-persistent-session (&optional node)
  "Return a persistent context-buffer session for NODE."
  (make-org-slipbox-buffer-session
   :kind 'persistent
   :current-node node
   :root-node node))

(defun org-slipbox-buffer--make-dedicated-session (node)
  "Return a dedicated context-buffer session rooted at NODE."
  (make-org-slipbox-buffer-session
   :kind 'dedicated
   :current-node node
   :root-node node))

(defun org-slipbox-buffer--session-node (&optional session)
  "Return the current node for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (org-slipbox-buffer-session-current-node session)))

(defun org-slipbox-buffer--clear-lens-cache (&optional session)
  "Clear cached exploration results for SESSION or the current buffer."
  (when-let ((session (or session org-slipbox-buffer-session)))
    (setf (org-slipbox-buffer-session-lens-cache session) nil)))

(defun org-slipbox-buffer--dedicated-name (node)
  "Return a dedicated context buffer name for NODE."
  (format "*org-slipbox: %s<%s>*"
          (plist-get node :title)
          (plist-get node :file_path)))

(defun org-slipbox-buffer--dedicated-p (&optional buffer)
  "Return non-nil when BUFFER is a dedicated org-slipbox buffer."
  (with-current-buffer (or buffer (current-buffer))
    (and (org-slipbox-buffer-session-p org-slipbox-buffer-session)
         (eq (org-slipbox-buffer-session-kind org-slipbox-buffer-session)
             'dedicated))))

(defun org-slipbox-buffer--render-expensive-sections-p ()
  "Return non-nil when expensive discovery sections should be rendered."
  (pcase org-slipbox-buffer-expensive-sections
    ('always t)
    ('dedicated (org-slipbox-buffer--dedicated-p))
    (_ nil)))

(defun org-slipbox-buffer--read-node-for-display ()
  "Read a node for dedicated buffer display."
  (or (org-slipbox-node-at-point)
      (let ((query (read-string "Node: ")))
        (or (org-slipbox-node-from-title-or-alias query)
            (let* ((response (org-slipbox-rpc-search-nodes
                              query
                              org-slipbox-search-limit))
                   (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
                   (choices (mapcar (lambda (candidate)
                                      (cons (org-slipbox--node-display candidate) candidate))
                                    nodes))
                   (selection (and choices
                                   (completing-read "Node: " choices nil t))))
              (and selection (cdr (assoc selection choices))))))))

(cl-defun org-slipbox-buffer-node-section (node &key heading)
  "Insert the current NODE summary section.
HEADING overrides the default section title, which is the node title."
  (let ((heading (or heading (plist-get node :title))))
    (org-slipbox-buffer--insert-heading heading)
    (org-slipbox-buffer--insert-metadata-line "File" (plist-get node :file_path))
    (when-let ((mtime (plist-get node :file_mtime_ns)))
      (org-slipbox-buffer--insert-metadata-line
       "Modified"
       (org-slipbox-buffer--format-file-mtime mtime)))
    (when-let ((outline (plist-get node :outline_path)))
      (unless (string-empty-p outline)
        (org-slipbox-buffer--insert-metadata-line "Outline" outline)))
    (when-let ((explicit-id (plist-get node :explicit_id)))
      (org-slipbox-buffer--insert-metadata-line "ID" explicit-id))
    (when-let ((backlink-count (plist-get node :backlink_count)))
      (org-slipbox-buffer--insert-metadata-line
       "Backlinks"
       (number-to-string backlink-count)))
    (when-let ((forward-link-count (plist-get node :forward_link_count)))
      (org-slipbox-buffer--insert-metadata-line
       "Forward Links"
       (number-to-string forward-link-count)))
    (when-let ((aliases (org-slipbox--plist-sequence (plist-get node :aliases))))
      (when aliases
        (org-slipbox-buffer--insert-metadata-line "Aliases" (string-join aliases ", "))))
    (when-let ((tags (org-slipbox--plist-sequence (plist-get node :tags))))
      (when tags
        (org-slipbox-buffer--insert-metadata-line "Tags" (string-join tags ", "))))
    (insert "\n")
    t))

(cl-defun org-slipbox-buffer-refs-section (node &key (section-heading "Refs"))
  "Insert the ref section for NODE using SECTION-HEADING."
  (let ((refs (org-slipbox--plist-sequence (plist-get node :refs))))
    (when refs
      (insert section-heading "\n")
      (insert (make-string (length section-heading) ?-) "\n")
      (dolist (reference refs)
        (insert reference "\n"))
      (insert "\n")
      t)))

(cl-defun org-slipbox-buffer-backlinks-section
    (node &key unique show-backlink-p (section-heading "Backlinks") (limit 200))
  "Insert a backlink section for NODE.
When UNIQUE is non-nil, only show the first backlink occurrence per
source node. SHOW-BACKLINK-P filters backlink entries when non-nil.
SECTION-HEADING overrides the rendered heading. LIMIT bounds the query."
  (let* ((backlinks (org-slipbox-buffer--backlinks node unique limit))
         (backlinks (if show-backlink-p
                        (seq-filter show-backlink-p backlinks)
                      backlinks)))
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     backlinks
     "No backlinks found."
     #'org-slipbox-buffer--insert-backlink-entry)))

(cl-defun org-slipbox-buffer-forward-links-section
    (node &key unique show-forward-link-p (section-heading "Forward Links") (limit 200))
  "Insert a forward-links section for NODE.
When UNIQUE is non-nil, only show the first forward-link occurrence per
destination node. SHOW-FORWARD-LINK-P filters forward-link entries when
non-nil. SECTION-HEADING overrides the rendered heading. LIMIT bounds the
query."
  (let* ((forward-links (org-slipbox-buffer--forward-links node unique limit))
         (forward-links (if show-forward-link-p
                            (seq-filter show-forward-link-p forward-links)
                          forward-links)))
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     forward-links
     "No forward links found."
     (lambda (entry)
       (org-slipbox-buffer--insert-forward-link-entry node entry)))))

(cl-defun org-slipbox-buffer-reflinks-section (node &key (section-heading "Reflinks"))
  "Insert a reflink section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     (org-slipbox-buffer--reflinks node)
     "No reflinks found."
     #'org-slipbox-buffer--insert-reflink-entry)))

(cl-defun org-slipbox-buffer-unlinked-references-section
    (node &key (section-heading "Unlinked References"))
  "Insert an unlinked-reference section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-occurrence-section
     section-heading
     (org-slipbox-buffer--unlinked-references node)
     "No unlinked references found."
     #'org-slipbox-buffer--insert-unlinked-reference-entry)))

(defun org-slipbox-buffer--forward-links (node &optional unique limit)
  "Return forward links for NODE.
When UNIQUE is non-nil, only return the first occurrence per destination
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'forward-links unique limit))

(defun org-slipbox-buffer--insert-heading (text)
  "Insert section heading TEXT."
  (insert text "\n")
  (insert (make-string (length text) ?=) "\n\n"))

(defun org-slipbox-buffer--insert-metadata-line (label value)
  "Insert LABEL and VALUE on one line."
  (insert (propertize (format "%-7s " (concat label ":")) 'face 'bold)
          value
          "\n"))

(defun org-slipbox-buffer--format-file-mtime (mtime-ns)
  "Return a display string for MTIME-NS."
  (format-time-string
   "%Y-%m-%d"
   (seconds-to-time (/ (float mtime-ns) 1000000000.0))))

(defun org-slipbox-buffer--insert-occurrence-section (title entries empty-message inserter)
  "Insert section TITLE using ENTRIES or EMPTY-MESSAGE via INSERTER."
  (insert title "\n")
  (insert (make-string (length title) ?-) "\n")
  (if entries
      (dolist (entry entries)
        (funcall inserter entry)
        (insert "\n"))
    (insert empty-message "\n"))
  (insert "\n")
  t)

(defun org-slipbox-buffer--insert-node-button (node)
  "Insert a button for NODE."
  (insert-text-button
   (org-slipbox--node-display node)
   'follow-link t
   'help-echo "Visit node"
   'action (lambda (_button)
             (org-slipbox--visit-node node))))

(defun org-slipbox-buffer--explanation-string (entry)
  "Return a display string for ENTRY's explanation payload."
  (when-let ((explanation (plist-get entry :explanation)))
    (pcase (plist-get explanation :kind)
      ("backlink" "direct backlink")
      ("forward-link" "direct forward link")
      ("shared-reference"
       (format "shared ref: %s" (plist-get explanation :reference)))
      ("unlinked-reference"
       (format "unlinked mention: %s"
               (plist-get explanation :matched_text)))
      ("shared-scheduled-date"
       (format "shared scheduled date: %s" (plist-get explanation :date)))
      ("shared-deadline-date"
       (format "shared deadline date: %s" (plist-get explanation :date)))
      ("shared-todo-keyword"
       (format "shared task state: %s"
               (plist-get explanation :todo_keyword))))))

(defun org-slipbox-buffer--insert-explanation (entry)
  "Insert ENTRY's explanation payload when it is present."
  (when-let ((reason (org-slipbox-buffer--explanation-string entry)))
    (insert " "
            (propertize reason 'face 'italic))))

(defun org-slipbox-buffer--insert-backlink-entry (entry)
  "Insert a preview-rich backlink ENTRY."
  (let* ((source-node (plist-get entry :source_note))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview)))
    (insert-text-button
     (org-slipbox--node-display source-node)
     'follow-link t
     'help-echo "Visit backlink"
     'action (lambda (_button)
               (org-slipbox-buffer--visit-location file row col)))
    (insert " "
            (propertize (format "%s:%s:%s" file row col) 'face 'shadow))
    (org-slipbox-buffer--insert-explanation entry)
    (insert
            "\n  "
            preview)))

(defun org-slipbox-buffer--insert-forward-link-entry (node entry)
  "Insert a preview-rich forward-link ENTRY for source NODE."
  (let* ((destination-node (plist-get entry :destination_note))
         (file (plist-get node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview)))
    (insert-text-button
     (org-slipbox--node-display destination-node)
     'follow-link t
     'help-echo "Visit linked node"
     'action (lambda (_button)
               (org-slipbox--visit-node destination-node)))
    (insert " "
            (propertize (format "%s:%s:%s" file row col) 'face 'shadow))
    (org-slipbox-buffer--insert-explanation entry)
    (insert
            "\n  "
            preview)))

(defun org-slipbox-buffer--insert-reflink-entry (entry)
  "Insert a preview-rich reflink ENTRY."
  (let* ((source-node (plist-get entry :source_anchor))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview))
         (matched-reference (plist-get entry :matched_reference)))
    (insert-text-button
     (org-slipbox--node-display source-node)
     'follow-link t
     'help-echo "Visit reflink source"
     'action (lambda (_button)
               (org-slipbox-buffer--visit-location file row col)))
    (insert " "
            (propertize (format "%s:%s:%s" file row col) 'face 'shadow))
    (org-slipbox-buffer--insert-explanation entry)
    (when (and (null (plist-get entry :explanation)) matched-reference)
      (insert " " (propertize matched-reference 'face 'italic)))
    (insert "\n  " preview)))

(defun org-slipbox-buffer--insert-unlinked-reference-entry (entry)
  "Insert a preview-rich unlinked-reference ENTRY."
  (let* ((source-node (plist-get entry :source_anchor))
         (file (plist-get source-node :file_path))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview))
         (matched-text (plist-get entry :matched_text)))
    (insert-text-button
     (org-slipbox--node-display source-node)
     'follow-link t
     'help-echo "Visit unlinked-reference source"
     'action (lambda (_button)
               (org-slipbox-buffer--visit-location file row col)))
    (insert " "
            (propertize (format "%s:%s:%s" file row col) 'face 'shadow))
    (org-slipbox-buffer--insert-explanation entry)
    (when (and (null (plist-get entry :explanation)) matched-text)
      (insert " " (propertize matched-text 'face 'italic)))
    (insert "\n  " preview)))

(defun org-slipbox-buffer--visit-location (file row col)
  "Visit FILE at ROW and COL."
  (find-file (if (file-name-absolute-p file)
                 file
               (expand-file-name file org-slipbox-directory)))
  (goto-char (point-min))
  (forward-line (1- row))
  (forward-char (1- col))
  (when (and (fboundp 'org-fold-show-context)
             (org-invisible-p))
    (org-fold-show-context)))

(defun org-slipbox-buffer--reflinks (node)
  "Return daemon-backed reflink matches for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'reflinks nil 200))

(defun org-slipbox-buffer--unlinked-references (node)
  "Return daemon-backed unlinked references for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'unlinked-references nil 200))

(defun org-slipbox-buffer--backlinks (node &optional unique limit)
  "Return backlinks for NODE.
When UNIQUE is non-nil, only return the first occurrence per source
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'backlinks unique limit))

(defun org-slipbox-buffer--exploration-section-kind-name (section-kind)
  "Return SECTION-KIND encoded for exploration results."
  (pcase section-kind
    ('backlinks "backlinks")
    ('forward-links "forward-links")
    ('reflinks "reflinks")
    ('unlinked-references "unlinked-references")
    ('time-neighbors "time-neighbors")
    ('task-neighbors "task-neighbors")
    (_
     (user-error "Unsupported exploration section kind %S" section-kind))))

(defun org-slipbox-buffer--exploration-cache-key (lens unique limit)
  "Return the cache key for exploration LENS, UNIQUE, and LIMIT."
  (list lens (or limit 200) (and unique t)))

(defun org-slipbox-buffer--exploration-result (node lens &optional unique limit)
  "Return cached exploration results for NODE under LENS."
  (let ((limit (or limit 200)))
    (when-let ((node-key (plist-get node :node_key)))
    (if-let* ((session org-slipbox-buffer-session)
              (cache-key (org-slipbox-buffer--exploration-cache-key lens unique limit))
              (cached (assoc cache-key
                             (org-slipbox-buffer-session-lens-cache session))))
        (cdr cached)
      (let ((result (org-slipbox-rpc-explore node-key lens limit unique)))
        (when-let ((session org-slipbox-buffer-session))
          (let ((cache-key (org-slipbox-buffer--exploration-cache-key lens unique limit))
                (existing (org-slipbox-buffer-session-lens-cache session)))
            (setf (org-slipbox-buffer-session-lens-cache session)
                  (cons (cons cache-key result)
                        (cl-remove cache-key existing :key #'car :test #'equal)))))
        result)))))

(defun org-slipbox-buffer--exploration-section-entries
    (node lens section-kind &optional unique limit)
  "Return SECTION-KIND entries for NODE under exploration LENS."
  (when-let* ((result (org-slipbox-buffer--exploration-result node lens unique limit))
              (sections (org-slipbox--plist-sequence (plist-get result :sections)))
              (section-name (org-slipbox-buffer--exploration-section-kind-name section-kind))
              (section (seq-find (lambda (candidate)
                                   (equal (plist-get candidate :kind) section-name))
                                 sections)))
    (org-slipbox--plist-sequence (plist-get section :entries))))

(provide 'org-slipbox-buffer)

;;; org-slipbox-buffer.el ends here
