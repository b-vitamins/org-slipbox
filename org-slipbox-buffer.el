;;; org-slipbox-buffer.el --- Context buffer for org-slipbox -*- lexical-binding: t; -*-

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
  "When grep-backed discovery sections should be rendered.
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

(defvar-local org-slipbox-buffer-current-node nil
  "Node currently rendered in the org-slipbox context buffer.")

(defconst org-slipbox-buffer--grep-result-re
  (rx bol
      (group (+ any))
      ":"
      (group (+ digit))
      ":"
      (group (+ digit))
      ":"
      (group (* any))
      eol)
  "Regexp for parsing `rg --vimgrep --only-matching' output.")

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
  (unless org-slipbox-buffer-current-node
    (user-error "No org-slipbox node to refresh"))
  (org-slipbox-buffer-render-contents))

;;;###autoload
(defun org-slipbox-buffer-display-dedicated (node)
  "Display a dedicated org-slipbox buffer for NODE."
  (interactive (list (org-slipbox-buffer--read-node-for-display)))
  (let ((buffer (get-buffer-create (org-slipbox-buffer--dedicated-name node))))
    (with-current-buffer buffer
      (setq-local org-slipbox-buffer-current-node node)
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
      (unless (equal node org-slipbox-buffer-current-node)
        (setq-local org-slipbox-buffer-current-node node)
        (org-slipbox-buffer-render-contents)
        (add-hook 'kill-buffer-hook #'org-slipbox-buffer--persistent-cleanup-h nil t)))))

(defun org-slipbox-buffer-render-contents ()
  "Render the current org-slipbox context buffer."
  (let* ((node org-slipbox-buffer-current-node)
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

(defun org-slipbox-buffer--dedicated-name (node)
  "Return a dedicated context buffer name for NODE."
  (format "*org-slipbox: %s<%s>*"
          (plist-get node :title)
          (plist-get node :file_path)))

(defun org-slipbox-buffer--dedicated-p (&optional buffer)
  "Return non-nil when BUFFER is a dedicated org-slipbox buffer."
  (string-prefix-p "*org-slipbox: "
                   (buffer-name (or buffer (current-buffer)))))

(defun org-slipbox-buffer--render-expensive-sections-p ()
  "Return non-nil when grep-backed sections should be rendered."
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
    (when-let ((outline (plist-get node :outline_path)))
      (unless (string-empty-p outline)
        (org-slipbox-buffer--insert-metadata-line "Outline" outline)))
    (when-let ((explicit-id (plist-get node :explicit_id)))
      (org-slipbox-buffer--insert-metadata-line "ID" explicit-id))
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

(cl-defun org-slipbox-buffer-reflinks-section (node &key (section-heading "Reflinks"))
  "Insert a reflink section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-result-section
     section-heading
     (org-slipbox-buffer--reflinks node)
     "No reflinks found.")))

(cl-defun org-slipbox-buffer-unlinked-references-section
    (node &key (section-heading "Unlinked References"))
  "Insert an unlinked-reference section for NODE using SECTION-HEADING."
  (when (org-slipbox-buffer--render-expensive-sections-p)
    (org-slipbox-buffer--insert-result-section
     section-heading
     (org-slipbox-buffer--unlinked-references node)
     "No unlinked references found.")))

(defun org-slipbox-buffer--backlinks (node &optional unique limit)
  "Return backlinks for NODE.
When UNIQUE is non-nil, only return the first occurrence per source
node. LIMIT bounds the number of rows requested."
  (let* ((response (org-slipbox-rpc-backlinks
                    (plist-get node :node_key)
                    (or limit 200)
                    unique))
         (backlinks (plist-get response :backlinks)))
    (org-slipbox--plist-sequence backlinks)))

(defun org-slipbox-buffer--insert-heading (text)
  "Insert section heading TEXT."
  (insert text "\n")
  (insert (make-string (length text) ?=) "\n\n"))

(defun org-slipbox-buffer--insert-metadata-line (label value)
  "Insert LABEL and VALUE on one line."
  (insert (propertize (format "%-7s " (concat label ":")) 'face 'bold)
          value
          "\n"))

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

(defun org-slipbox-buffer--insert-backlink-entry (entry)
  "Insert a preview-rich backlink ENTRY."
  (let* ((source-node (plist-get entry :source_node))
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
            (propertize (format "%s:%s:%s" file row col) 'face 'shadow)
            "\n  "
            preview)))

(defun org-slipbox-buffer--insert-result-section (title results empty-message)
  "Insert result section TITLE using RESULTS or EMPTY-MESSAGE."
  (org-slipbox-buffer--insert-occurrence-section
   title
   results
   empty-message
   #'org-slipbox-buffer--insert-result-entry))

(defun org-slipbox-buffer--insert-result-entry (entry)
  "Insert a preview-rich grep ENTRY."
  (let* ((file (plist-get entry :file))
         (row (plist-get entry :row))
         (col (plist-get entry :col))
         (preview (plist-get entry :preview))
         (label (format "%s:%s:%s"
                        (file-relative-name file org-slipbox-directory)
                        row
                        col)))
    (insert-text-button
     label
     'follow-link t
     'help-echo "Visit result"
     'action (lambda (_button)
               (org-slipbox-buffer--visit-result entry)))
    (insert "\n  " preview)))

(defun org-slipbox-buffer--visit-result (entry)
  "Visit grep result ENTRY."
  (org-slipbox-buffer--visit-location
   (plist-get entry :file)
   (plist-get entry :row)
   (plist-get entry :col)))

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
  "Return grep-backed reflink matches for NODE."
  (let* ((refs (org-slipbox--plist-sequence (plist-get node :refs)))
         (patterns (and refs (org-slipbox-buffer--reflink-patterns refs))))
    (when (and patterns (executable-find "rg"))
      (org-slipbox-buffer--grep-results
       (org-slipbox-buffer--with-pattern-file
        patterns
        (lambda (pattern-file)
          (shell-command-to-string
           (org-slipbox-buffer--reflinks-rg-command patterns pattern-file))))
       node))))

(defun org-slipbox-buffer--unlinked-references (node)
  "Return grep-backed unlinked references for NODE."
  (let ((titles (delete-dups
                 (append (list (plist-get node :title))
                         (org-slipbox--plist-sequence (plist-get node :aliases))))))
    (when (and (executable-find "rg")
               (org-slipbox-buffer--rg-supports-pcre2-p)
               titles)
      (seq-filter
       (lambda (entry)
         (org-slipbox-buffer--member-ignore-case-p
          (plist-get entry :match)
          titles))
       (org-slipbox-buffer--grep-results
        (org-slipbox-buffer--with-pattern-file
         (list
          (concat "\\[([^[]]++|(?R))*\\]"
                  (mapconcat
                   (lambda (title)
                     (format "|(\\b%s\\b)" (regexp-quote title)))
                   titles
                   "")))
         (lambda (pattern-file)
           (shell-command-to-string
            (org-slipbox-buffer--unlinked-rg-command titles pattern-file))))
        node)))))

(defun org-slipbox-buffer--reflink-patterns (refs)
  "Return fixed-string search patterns derived from REFS."
  (let (patterns)
    (dolist (reference refs)
      (push reference patterns)
      (when (string-prefix-p "@" reference)
        (push (concat "cite:" (substring reference 1)) patterns)))
    (delete-dups (nreverse patterns))))

(defun org-slipbox-buffer--reflinks-rg-command (_patterns pattern-file)
  "Return ripgrep command for reflinks using PATTERN-FILE."
  (format
   "rg --follow --only-matching --vimgrep --ignore-case --fixed-strings %s --file %s %s"
   (org-slipbox-buffer--rg-glob-arguments)
   (shell-quote-argument pattern-file)
   (shell-quote-argument (expand-file-name org-slipbox-directory))))

(defun org-slipbox-buffer--unlinked-rg-command (_titles pattern-file)
  "Return ripgrep command for unlinked references using PATTERN-FILE."
  (format
   "rg --follow --only-matching --vimgrep --pcre2 --ignore-case %s --file %s %s"
   (org-slipbox-buffer--rg-glob-arguments)
   (shell-quote-argument pattern-file)
   (shell-quote-argument (expand-file-name org-slipbox-directory))))

(defun org-slipbox-buffer--rg-glob-arguments ()
  "Return shell-quoted ripgrep glob arguments for eligible files."
  (mapconcat (lambda (glob)
               (format "--glob %s" (shell-quote-argument glob)))
             (org-slipbox--file-globs)
             " "))

(defun org-slipbox-buffer--grep-results (command node)
  "Run grep COMMAND and return filtered results for NODE."
  (let* ((range (org-slipbox-buffer--node-line-range node))
         (output (shell-command-to-string command))
         results)
    (dolist (line (split-string output "\n" t))
      (when-let ((entry (org-slipbox-buffer--parse-grep-result line)))
        (unless (org-slipbox-buffer--entry-in-node-p entry node range)
          (push entry results))))
    (nreverse (cl-delete-duplicates results :test #'equal))))

(defun org-slipbox-buffer--parse-grep-result (line)
  "Parse a ripgrep LINE into a result plist."
  (when (string-match org-slipbox-buffer--grep-result-re line)
    (let ((file (match-string 1 line))
          (row (string-to-number (match-string 2 line)))
          (col (string-to-number (match-string 3 line)))
          (match (match-string 4 line)))
      (list :file file
            :row row
            :col col
            :match match
            :preview (org-slipbox-buffer--result-preview-line file row)))))

(defun org-slipbox-buffer--result-preview-line (file row)
  "Return preview text from FILE at ROW."
  (with-temp-buffer
    (insert-file-contents file)
    (forward-line (1- row))
    (string-trim-right
     (buffer-substring-no-properties
      (line-beginning-position)
      (line-end-position)))))

(defun org-slipbox-buffer--entry-in-node-p (entry node range)
  "Return non-nil when ENTRY falls within NODE RANGE."
  (and (equal (expand-file-name (plist-get entry :file))
              (expand-file-name (plist-get node :file_path) org-slipbox-directory))
       (<= (car range) (plist-get entry :row))
       (<= (plist-get entry :row) (cdr range))))

(defun org-slipbox-buffer--node-line-range (node)
  "Return the inclusive line range occupied by NODE."
  (let ((absolute-file (expand-file-name (plist-get node :file_path) org-slipbox-directory)))
    (if (equal (plist-get node :kind) "file")
        (cons 1 most-positive-fixnum)
      (with-temp-buffer
        (insert-file-contents absolute-file)
        (org-mode)
        (goto-char (point-min))
        (forward-line (1- (plist-get node :line)))
        (cons (plist-get node :line)
              (save-excursion
                (org-end-of-subtree t t)
                (line-number-at-pos)))))))

(defun org-slipbox-buffer--with-pattern-file (patterns builder)
  "Call BUILDER with a temporary pattern file containing PATTERNS."
  (let ((pattern-file (make-temp-file "org-slipbox-rg-pattern-")))
    (unwind-protect
        (progn
          (with-temp-file pattern-file
            (insert (string-join patterns "\n")))
          (funcall builder pattern-file))
      (when (file-exists-p pattern-file)
        (delete-file pattern-file)))))

(defun org-slipbox-buffer--rg-supports-pcre2-p ()
  "Return non-nil when the installed `rg' supports PCRE2."
  (not (string-match-p
        "PCRE2 is not available"
        (shell-command-to-string "rg --pcre2-version"))))

(defun org-slipbox-buffer--member-ignore-case-p (item items)
  "Return non-nil when ITEM case-insensitively matches an element of ITEMS."
  (let ((needle (downcase item)))
    (cl-some (lambda (candidate)
               (string-equal needle (downcase candidate)))
             items)))

(provide 'org-slipbox-buffer)

;;; org-slipbox-buffer.el ends here
