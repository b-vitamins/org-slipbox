;;; org-slipbox-node.el --- Node commands for org-slipbox -*- lexical-binding: t; -*-

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

;; Node indexing and lookup commands for `org-slipbox'.

;;; Code:

(require 'org)
(require 'org-id)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-search-limit 50
  "Maximum number of nodes to request for interactive search."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-tag-search-limit 200
  "Maximum number of indexed tags to request per completion query."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-link-type "slipbox"
  "Org link type used for title-based org-slipbox links."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-link-auto-replace nil
  "When non-nil, replace `org-slipbox-link-type' links with `id:' links on save."
  :type 'boolean
  :group 'org-slipbox)

(defcustom org-slipbox-completion-everywhere nil
  "When non-nil, complete words at point into org-slipbox links."
  :type 'boolean
  :group 'org-slipbox)

(defcustom org-slipbox-extract-file-name-template "${slug}.org"
  "Default relative file target template for `org-slipbox-extract-subtree'."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-capture-templates
  '(("d" "default" :path "${slug}.org" :title "${title}"))
  "Capture templates for `org-slipbox-capture'.
Each template is a list of the form (KEY DESCRIPTION [:path STRING] [:title STRING])."
  :type 'sexp
  :group 'org-slipbox)

(org-link-set-parameters org-slipbox-link-type :follow #'org-slipbox-link-follow-link)

(defconst org-slipbox-bracket-completion-re
  "\\[\\[\\(\\(?:slipbox:\\)?\\)\\([^]]*\\)]]"
  "Regexp for completion within Org bracket links.")

(define-minor-mode org-slipbox-completion-mode
  "Enable org-slipbox completion and link replacement in the current buffer."
  :lighter " Slipbox"
  (if org-slipbox-completion-mode
      (progn
        (add-hook 'completion-at-point-functions #'org-slipbox-complete-link-at-point nil t)
        (add-hook 'completion-at-point-functions #'org-slipbox-complete-everywhere nil t)
        (add-hook 'before-save-hook #'org-slipbox--replace-slipbox-links-on-save-h nil t))
    (remove-hook 'completion-at-point-functions #'org-slipbox-complete-link-at-point t)
    (remove-hook 'completion-at-point-functions #'org-slipbox-complete-everywhere t)
    (remove-hook 'before-save-hook #'org-slipbox--replace-slipbox-links-on-save-h t)))

(defun org-slipbox-index ()
  "Rebuild the local org-slipbox index from Org files."
  (interactive)
  (let* ((response (org-slipbox-rpc-request "slipbox/index"))
         (files (plist-get response :files_indexed))
         (nodes (plist-get response :nodes_indexed))
         (links (plist-get response :links_indexed)))
    (message "Indexed %s files, %s nodes, %s links" files nodes links)
    response))

(defun org-slipbox-node-from-id (id)
  "Return the indexed node with explicit ID."
  (org-slipbox-rpc-request "slipbox/nodeFromId" `(:id ,id)))

(defun org-slipbox-node-from-title-or-alias (title-or-alias &optional nocase)
  "Return the indexed node matching TITLE-OR-ALIAS.
When NOCASE is non-nil, use case-insensitive matching."
  (org-slipbox-rpc-request
   "slipbox/nodeFromTitleOrAlias"
   `(:title_or_alias ,title-or-alias :nocase ,(and nocase t))))

(defun org-slipbox-node-at-point (&optional assert)
  "Return the indexed node at point.
If ASSERT is non-nil, signal a user error when no node is available."
  (let ((node (and (org-slipbox--queryable-node-buffer-p)
                   (progn
                     (org-slipbox--sync-node-buffer-if-needed)
                     (org-slipbox-rpc-request
                      "slipbox/nodeAtPoint"
                      `(:file_path ,(expand-file-name (org-slipbox--current-node-buffer-file))
                        :line ,(line-number-at-pos)))))))
    (or node
        (and assert (user-error "No node at point")))))

(defun org-slipbox-link-follow-link (title-or-alias)
  "Visit the node named by TITLE-OR-ALIAS."
  (let ((node (or (org-slipbox-node-from-title-or-alias title-or-alias)
                  (org-slipbox--capture-node title-or-alias))))
    (when org-slipbox-link-auto-replace
      (org-slipbox-link-replace-at-point))
    (org-mark-ring-push)
    (org-slipbox--visit-node node)))

(defun org-slipbox-link-replace-at-point (&optional link)
  "Replace `org-slipbox-link-type' LINK at point with an `id:' link."
  (save-excursion
    (save-match-data
      (let* ((link (or link (org-element-context)))
             (type (org-element-property :type link))
             (path (org-element-property :path link))
             (description (and (org-element-property :contents-begin link)
                               (org-element-property :contents-end link)
                               (buffer-substring-no-properties
                                (org-element-property :contents-begin link)
                                (org-element-property :contents-end link))))
             node)
        (goto-char (org-element-property :begin link))
        (when (and (org-in-regexp org-link-any-re 1)
                   (string-equal type org-slipbox-link-type)
                   (setq node (save-match-data
                                (org-slipbox-node-from-title-or-alias path))))
          (let* ((node-with-id (org-slipbox--ensure-node-id node))
                 (explicit-id (plist-get node-with-id :explicit_id)))
            (replace-match (org-link-make-string
                            (concat "id:" explicit-id)
                            (or description path)))))))))

(defun org-slipbox-link-replace-all ()
  "Replace all `org-slipbox-link-type' links in the current buffer."
  (interactive)
  (org-with-point-at 1
    (while (search-forward (format "[[%s:" org-slipbox-link-type) nil t)
      (org-slipbox-link-replace-at-point))))

(defun org-slipbox-complete-link-at-point ()
  "Complete `org-slipbox-link-type' links at point."
  (let (slipbox-p start end)
    (when (org-in-regexp org-slipbox-bracket-completion-re 1)
      (setq slipbox-p (not (or (org-in-src-block-p)
                               (string-blank-p (match-string 1))))
            start (match-beginning 2)
            end (match-end 2))
      (list start end
            #'org-slipbox--title-completion-table
            :exit-function
            (lambda (string &rest _)
              (delete-char (- (length string)))
              (insert (concat (unless slipbox-p
                                (concat org-slipbox-link-type ":"))
                              string))
              (forward-char 2))))))

(defun org-slipbox-complete-everywhere ()
  "Complete words at point into org-slipbox links."
  (when (and org-slipbox-completion-everywhere
             (thing-at-point 'word)
             (not (org-in-src-block-p))
             (not (save-match-data (org-in-regexp org-link-any-re))))
    (let ((bounds (bounds-of-thing-at-point 'word)))
      (list (car bounds) (cdr bounds)
            #'org-slipbox--title-completion-table
            :exit-function
            (lambda (string _status)
              (delete-char (- (length string)))
              (insert "[[" org-slipbox-link-type ":" string "]]"))
            :exclusive 'no))))

;;;###autoload
(defun org-slipbox-refile (node)
  "Refile the current subtree under NODE."
  (interactive (list (org-slipbox--read-existing-node "Refile to: ")))
  (unless node
    (user-error "No target node selected"))
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (let* ((source-buffer (current-buffer))
         (source-file (expand-file-name buffer-file-name))
         (source-node (org-slipbox-node-at-point t))
         source-start
         source-end
         subtree-text
         target-buffer
         target-point)
    (when (equal (plist-get source-node :node_key)
                 (plist-get node :node_key))
      (user-error "Target is the same as current node"))
    (save-excursion
      (save-restriction
        (widen)
        (when (org-slipbox--file-node-p)
          (org-slipbox-demote-entire-buffer))
        (org-back-to-heading t)
        (setq source-start (copy-marker (point)))
        (setq source-end (copy-marker (save-excursion (org-end-of-subtree t t))))
        (setq subtree-text
              (buffer-substring-no-properties source-start source-end))))
    (setq target-buffer
          (find-file-noselect
           (expand-file-name (plist-get node :file_path) org-slipbox-directory)))
    (with-current-buffer target-buffer
      (save-excursion
        (save-restriction
          (widen)
          (setq target-point (org-slipbox--node-point node))
          (when (and (eq source-buffer target-buffer)
                     (>= target-point source-start)
                     (< target-point source-end))
            (user-error "Target is inside the current subtree"))
          (org-slipbox--paste-subtree-into-node node subtree-text))))
    (with-current-buffer source-buffer
      (save-excursion
        (save-restriction
          (widen)
          (org-preserve-local-variables
           (delete-region source-start source-end)))))
    (if (org-slipbox--buffer-empty-p source-buffer)
        (progn
          (when (buffer-live-p source-buffer)
            (with-current-buffer source-buffer
              (set-buffer-modified-p nil))
            (kill-buffer source-buffer))
          (delete-file source-file)
          (org-slipbox--sync-file-path source-file))
      (org-slipbox--save-and-sync-buffer source-buffer))
    (unless (eq source-buffer target-buffer)
      (org-slipbox--save-and-sync-buffer target-buffer))
    node))

;;;###autoload
(defun org-slipbox-extract-subtree (&optional file-path)
  "Extract the current subtree into FILE-PATH under `org-slipbox-directory'."
  (interactive)
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (save-excursion
    (save-restriction
      (widen)
      (when (org-slipbox--file-node-p)
        (user-error "Already a top-level node"))
      (org-back-to-heading t)
      (org-id-get-create)
      (let* ((title (nth 4 (org-heading-components)))
             (target-path (org-slipbox--extract-target-path title file-path))
             (source-buffer (current-buffer))
             (target-buffer nil)
             (source-start (copy-marker (point)))
             (source-end (copy-marker (save-excursion (org-end-of-subtree t t))))
             (subtree-text (buffer-substring-no-properties source-start source-end)))
        (when (file-exists-p target-path)
          (user-error "%s exists. Aborting" target-path))
        (setq target-buffer (find-file-noselect target-path))
        (org-preserve-local-variables
         (delete-region source-start source-end))
        (org-slipbox--save-and-sync-buffer source-buffer)
        (with-current-buffer target-buffer
          (save-excursion
            (save-restriction
              (widen)
              (erase-buffer)
              (org-mode)
              (org-slipbox--paste-subtree-into-node
               '(:kind "file" :line 1)
               subtree-text)
              (goto-char (point-min))
              (org-back-to-heading t)
              (while (> (org-current-level) 1)
                (org-promote-subtree))
              (org-slipbox--promote-entire-buffer-internal)))
          (org-slipbox--save-and-sync-buffer target-buffer))
        target-path))))

(defun org-slipbox-capture (&optional title)
  "Create a new note with TITLE and visit it."
  (interactive)
  (let* ((title (or title (read-string "Capture title: ")))
         (template (org-slipbox--read-capture-template)))
    (org-slipbox--visit-node (org-slipbox--capture-node title template))))

(defun org-slipbox-node-find (query)
  "Find a node matching QUERY and visit it."
  (interactive (list (read-string "Find node: ")))
  (let ((node (org-slipbox--select-or-capture-node query)))
    (when node
      (org-slipbox--visit-node node))))

(defun org-slipbox-node-insert (query)
  "Insert an `id:' link to a node selected using QUERY."
  (interactive (list (read-string "Insert node: ")))
  (let* ((node (org-slipbox--select-or-capture-node query))
         (node-with-id (and node (org-slipbox--ensure-node-id node)))
         (id (and node-with-id (plist-get node-with-id :explicit_id)))
         (title (and node-with-id (plist-get node-with-id :title))))
    (when node-with-id
      (insert (format "[[id:%s][%s]]" id title)))))

(defun org-slipbox-node-backlinks (query)
  "Show backlinks for a node selected using QUERY."
  (interactive (list (read-string "Backlinks for: ")))
  (let* ((node (org-slipbox--select-or-capture-node query))
         (node-key (plist-get node :node_key))
         (response (and node
                        (org-slipbox-rpc-request
                         "slipbox/backlinks"
                         `(:node_key ,node-key :limit 200))))
         (backlinks (and response
                         (org-slipbox--plist-sequence
                          (plist-get response :backlinks)))))
    (when node
      (with-current-buffer (get-buffer-create "*org-slipbox backlinks*")
        (let ((inhibit-read-only t))
          (erase-buffer)
          (special-mode)
          (insert (format "Backlinks for %s\n\n" (plist-get node :title)))
          (if backlinks
              (dolist (backlink backlinks)
                (insert (org-slipbox--node-display backlink) "\n"))
            (insert "No backlinks found.\n")))
        (display-buffer (current-buffer))))))

(defun org-slipbox-node-from-ref (reference)
  "Return the indexed node for REFERENCE, or nil when none exists."
  (org-slipbox-rpc-request "slipbox/nodeFromRef" `(:reference ,reference)))

(defun org-slipbox-ref-find (query)
  "Find a node by reference QUERY and visit it."
  (interactive (list (read-string "Find ref: ")))
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchRefs"
                    `(:query ,query :limit ,org-slipbox-search-limit)))
         (refs (org-slipbox--plist-sequence (plist-get response :refs)))
         (choices (mapcar (lambda (entry)
                            (cons (org-slipbox--ref-display entry) entry))
                          refs)))
    (cond
     ((null refs)
      (user-error "No ref matches %s" query))
     ((= (length refs) 1)
      (org-slipbox--visit-node (plist-get (car refs) :node)))
     (t
      (let* ((selection (completing-read "Ref: " choices nil t))
             (entry (cdr (assoc selection choices))))
        (org-slipbox--visit-node (plist-get entry :node)))))))

(defun org-slipbox-ref-add (reference)
  "Add REFERENCE to the current node."
  (interactive (list (read-string "Ref: ")))
  (org-slipbox--node-property-add "ROAM_REFS" reference))

(defun org-slipbox-ref-remove (&optional reference)
  "Remove REFERENCE from the current node."
  (interactive)
  (let* ((references (org-slipbox--current-node-property-values "ROAM_REFS"))
         (reference (or reference
                        (and references
                             (completing-read "Ref: " references nil t)))))
    (unless reference
      (user-error "No ref to remove"))
    (org-slipbox--node-property-remove "ROAM_REFS" reference)))

(defun org-slipbox-alias-add (alias)
  "Add ALIAS to the current node."
  (interactive (list (read-string "Alias: ")))
  (org-slipbox--node-property-add "ROAM_ALIASES" alias))

(defun org-slipbox-alias-remove (&optional alias)
  "Remove ALIAS from the current node."
  (interactive)
  (let* ((aliases (org-slipbox--current-node-property-values "ROAM_ALIASES"))
         (alias (or alias
                    (and aliases
                         (completing-read "Alias: " aliases nil t)))))
    (unless alias
      (user-error "No alias to remove"))
    (org-slipbox--node-property-remove "ROAM_ALIASES" alias)))

(defun org-slipbox-tag-completions (&optional prefix)
  "Return known tags for completion.
When PREFIX is non-nil, only return tags matching PREFIX."
  (delete-dups
   (append (org-slipbox--indexed-tags (or prefix "") 10000)
           (org-slipbox--matching-org-tags prefix))))

;;;###autoload
(defun org-slipbox-tag-add (tags)
  "Add TAGS to the current node."
  (interactive
   (list
    (let ((crm-separator "[ \t]*:[ \t]*"))
      (completing-read-multiple "Tag: " #'org-slipbox--tag-completion-table nil nil))))
  (setq tags (delete-dups (seq-filter #'identity (mapcar #'string-trim tags))))
  (unless tags
    (user-error "No tag to add"))
  (org-slipbox--set-current-node-tags
   (delete-dups (append tags (org-slipbox--current-node-tags))))
  tags)

;;;###autoload
(defun org-slipbox-tag-remove (&optional tags)
  "Remove TAGS from the current node."
  (interactive)
  (let* ((current-tags (org-slipbox--current-node-tags))
         (tags (or tags
                   (and current-tags
                        (completing-read-multiple "Tag: " current-tags nil t)))))
    (unless current-tags
      (user-error "No tag to remove"))
    (setq tags (delete-dups (seq-filter #'identity (mapcar #'string-trim tags))))
    (unless tags
      (user-error "No tag selected"))
    (org-slipbox--set-current-node-tags
     (seq-remove (lambda (tag) (member tag tags)) current-tags))
    tags))

(defun org-slipbox--select-or-capture-node (query)
  "Return a node selected for QUERY, or create one."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchNodes"
                    `(:query ,query :limit ,org-slipbox-search-limit)))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
         (create-choice (format "[Create] %s" query))
         (choices (mapcar (lambda (node)
                            (cons (org-slipbox--node-display node) node))
                          nodes))
         (collection (append choices (list (cons create-choice :create))))
         (selection (completing-read "Node: " collection nil t nil nil create-choice))
         (choice (cdr (assoc selection collection))))
    (cond
     ((eq choice :create) (org-slipbox--capture-node query))
     (choice choice)
     (t nil))))

(defun org-slipbox--ref-display (entry)
  "Return a display string for reference ENTRY."
  (format "%s | %s"
          (plist-get entry :reference)
          (org-slipbox--node-display (plist-get entry :node))))

(defun org-slipbox--capture-node (title &optional template)
  "Capture a new node with TITLE using TEMPLATE."
  (let* ((template (or template (org-slipbox--default-capture-template)))
         (template-options (and template (nthcdr 2 template)))
         (current-time (current-time))
         (capture-title (or (org-slipbox--expand-capture-template
                             (plist-get template-options :title)
                             title
                             current-time)
                            title))
         (file-path (org-slipbox--expand-capture-template
                     (plist-get template-options :path)
                     title
                     current-time)))
    (org-slipbox-rpc-request
     "slipbox/captureNode"
     (if file-path
         `(:title ,capture-title :file_path ,file-path)
       `(:title ,capture-title)))))

(defun org-slipbox--ensure-node-id (node)
  "Return NODE with an explicit ID, assigning one if needed."
  (if (plist-get node :explicit_id)
      node
    (org-slipbox-rpc-request
     "slipbox/ensureNodeId"
     `(:node_key ,(plist-get node :node_key)))))

(defun org-slipbox--node-display (node)
  "Return a display string for NODE."
  (let ((title (plist-get node :title))
        (outline (plist-get node :outline_path))
        (tags (org-slipbox--plist-sequence (plist-get node :tags)))
        (file (plist-get node :file_path))
        (line (plist-get node :line)))
    (string-join
     (delq nil
           (list title
                 (unless (string-empty-p outline) outline)
                 (unless (null tags)
                   (string-join (mapcar (lambda (tag) (format "#%s" tag)) tags) " "))
                 (format "%s:%s" file line)))
     " | ")))

(defun org-slipbox--plist-sequence (value)
  "Normalize JSON-derived VALUE into an Emacs list."
  (cond
   ((null value) nil)
   ((vectorp value) (append value nil))
   ((listp value) value)
   (t (list value))))

(defun org-slipbox--default-capture-template ()
  "Return the default capture template."
  (car org-slipbox-capture-templates))

(defun org-slipbox--read-capture-template ()
  "Prompt for a capture template when more than one is configured."
  (cond
   ((null org-slipbox-capture-templates) nil)
   ((= (length org-slipbox-capture-templates) 1)
    (car org-slipbox-capture-templates))
   (t
    (let* ((choices (mapcar (lambda (template)
                              (cons (format "%s %s" (car template) (cadr template))
                                    template))
                            org-slipbox-capture-templates))
           (selection (completing-read "Template: " choices nil t)))
      (cdr (assoc selection choices))))))

(defun org-slipbox--expand-capture-template (template title time)
  "Expand TEMPLATE for TITLE using TIME."
  (when template
    (let ((expanded (replace-regexp-in-string
                     "%<\\([^>]+\\)>"
                     (lambda (match)
                       (format-time-string
                        (substring match 2 -1)
                        time))
                     template
                     t)))
      (setq expanded
            (replace-regexp-in-string
             (regexp-quote "${title}")
             title
             expanded
             t
             t))
      (replace-regexp-in-string
       (regexp-quote "${slug}")
       (org-slipbox--slugify title)
       expanded
       t
       t))))

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

(defun org-slipbox--node-property-add (property value)
  "Add VALUE to PROPERTY on the current node."
  (setq value (string-trim value))
  (when (string-empty-p value)
    (user-error "%s must not be empty" property))
  (let* ((current (org-slipbox--current-node-property-values property))
         (updated (if (member value current) current (append current (list value)))))
    (org-slipbox--set-current-node-property property updated)))

(defun org-slipbox--node-property-remove (property value)
  "Remove VALUE from PROPERTY on the current node."
  (let* ((current (org-slipbox--current-node-property-values property))
         (updated (delete value (copy-sequence current))))
    (org-slipbox--set-current-node-property property updated)))

(defun org-slipbox--current-node-property-values (property)
  "Return PROPERTY values from the current node."
  (save-excursion
    (save-restriction
      (widen)
      (if (org-slipbox--file-node-p)
          (org-slipbox--file-property-values property)
        (org-back-to-heading t)
        (org-slipbox--split-property-values (org-entry-get (point) property))))))

(defun org-slipbox--set-current-node-property (property values)
  "Set PROPERTY on the current node to VALUES."
  (save-excursion
    (save-restriction
      (widen)
      (if (org-slipbox--file-node-p)
          (org-slipbox--set-file-property property values)
        (org-back-to-heading t)
        (if values
            (org-entry-put (point) property (org-slipbox--format-property-values values))
          (org-delete-property property)))))
  (org-slipbox--save-and-sync-current-buffer)
  values)

(defun org-slipbox--file-node-p ()
  "Return non-nil when point refers to the file-level node."
  (org-before-first-heading-p))

(defun org-slipbox--file-property-values (property)
  "Return file-level PROPERTY values."
  (pcase-let ((`(,start . ,end) (or (org-slipbox--file-property-drawer-bounds)
                                    '(nil . nil))))
    (if (and start end)
        (save-excursion
          (goto-char start)
          (if (re-search-forward
               (format "^[ \t]*:%s:[ \t]*\\(.*\\)$" (regexp-quote property))
               end
               t)
              (org-slipbox--split-property-values (match-string 1))
            nil))
      nil)))

(defun org-slipbox--set-file-property (property values)
  "Set file-level PROPERTY to VALUES."
  (let ((value (and values (org-slipbox--format-property-values values))))
    (pcase-let ((`(,drawer-start . ,drawer-end) (or (org-slipbox--file-property-drawer-bounds)
                                                    '(nil . nil))))
      (if (and drawer-start drawer-end)
          (save-excursion
            (goto-char drawer-start)
            (if (re-search-forward
                 (format "^[ \t]*:%s:[ \t]*\\(.*\\)$" (regexp-quote property))
                 drawer-end
                 t)
                (if value
                    (replace-match (format ":%s: %s" property value) t t)
                  (delete-region (line-beginning-position)
                                 (min (point-max) (1+ (line-end-position))))
                  (when (org-slipbox--file-property-drawer-empty-p drawer-start)
                    (org-slipbox--delete-file-property-drawer drawer-start)))
              (when value
                (goto-char drawer-end)
                (forward-line -1)
                (beginning-of-line)
                (insert (format ":%s: %s\n" property value)))))
        (when value
          (save-excursion
            (goto-char (org-slipbox--file-property-insert-point))
            (insert ":PROPERTIES:\n"
                    (format ":%s: %s\n" property value)
                    ":END:\n")
            (unless (looking-at-p "\n\\|\\'")
              (insert "\n"))))))))

(defun org-slipbox--file-property-drawer-bounds ()
  "Return the bounds of the file-level property drawer, if present."
  (save-excursion
    (goto-char (point-min))
    (goto-char (org-slipbox--file-property-insert-point))
    (while (and (not (eobp)) (looking-at-p "[ \t]*$"))
      (forward-line 1))
    (when (looking-at-p "[ \t]*:PROPERTIES:[ \t]*$")
      (let ((start (line-beginning-position)))
        (when (re-search-forward "^[ \t]*:END:[ \t]*$" nil t)
          (cons start (line-end-position)))))))

(defun org-slipbox--file-property-insert-point ()
  "Return the buffer position where the file property drawer belongs."
  (save-excursion
    (goto-char (point-min))
    (while (and (not (eobp)) (looking-at-p "[ \t]*$"))
      (forward-line 1))
    (while (and (not (eobp))
                (string-prefix-p "#+" (string-trim (or (thing-at-point 'line t) ""))))
      (forward-line 1))
    (point)))

(defun org-slipbox--file-property-drawer-empty-p (drawer-start)
  "Return non-nil when the file property drawer at DRAWER-START has no entries."
  (save-excursion
    (goto-char drawer-start)
    (forward-line 1)
    (looking-at-p "[ \t]*:END:[ \t]*$")))

(defun org-slipbox--delete-file-property-drawer (drawer-start)
  "Delete the file property drawer starting at DRAWER-START."
  (save-excursion
    (goto-char drawer-start)
    (when (re-search-forward "^[ \t]*:END:[ \t]*$" nil t)
      (delete-region drawer-start
                     (min (point-max)
                          (if (eobp) (point) (1+ (point))))))))

(defun org-slipbox--split-property-values (value)
  "Split multivalue property VALUE into a list."
  (when (and value (not (string-empty-p value)))
    (let ((values nil)
          (current "")
          (in-quotes nil)
          (escape nil)
          (bracket-depth 0))
      (dolist (character (string-to-list value))
        (cond
         (escape
          (setq current (concat current (string character))
                escape nil))
         ((and in-quotes (eq character ?\\))
          (setq escape t))
         ((eq character ?\")
          (setq in-quotes (not in-quotes)))
         ((and (not in-quotes) (eq character ?\[))
          (setq bracket-depth (1+ bracket-depth)
                current (concat current (string character))))
         ((and (not in-quotes) (eq character ?\]))
          (setq bracket-depth (max 0 (1- bracket-depth))
                current (concat current (string character))))
         ((and (not in-quotes) (= bracket-depth 0) (memq character '(?\s ?\t ?\n)))
          (unless (string-empty-p current)
            (push current values)
            (setq current "")))
         (t
          (setq current (concat current (string character))))))
      (unless (string-empty-p current)
        (push current values))
      (nreverse values))))

(defun org-slipbox--format-property-values (values)
  "Format property VALUES as a single Org property string."
  (mapconcat
   (lambda (value)
     (if (string-match-p "[[:space:]\"]" value)
         (prin1-to-string value)
       value))
   values
   " "))

(defun org-slipbox--save-and-sync-current-buffer ()
  "Save and sync the current buffer into the index."
  (unless buffer-file-name
    (user-error "Current buffer is not visiting a file"))
  (org-slipbox--save-and-sync-buffer (current-buffer)))

(defun org-slipbox--save-and-sync-buffer (buffer)
  "Save BUFFER and sync it into the index."
  (with-current-buffer buffer
    (unless buffer-file-name
      (user-error "Current buffer is not visiting a file"))
    (save-buffer)
    (org-slipbox--sync-file-path buffer-file-name)))

(defun org-slipbox--sync-file-path (path)
  "Sync absolute PATH into the index."
  (org-slipbox-rpc-request
   "slipbox/indexFile"
   `(:file_path ,(expand-file-name path))))

(defun org-slipbox--node-point (node)
  "Return the point for NODE in the current buffer."
  (save-excursion
    (goto-char (point-min))
    (forward-line (1- (plist-get node :line)))
    (point)))

(defun org-slipbox--paste-subtree-into-node (node subtree-text)
  "Paste SUBTREE-TEXT under NODE."
  (goto-char (org-slipbox--node-point node))
  (let ((kill-ring (list subtree-text))
        (kill-ring-yank-pointer nil))
    (setq kill-ring-yank-pointer kill-ring)
    (if (equal (plist-get node :kind) "file")
        (progn
          (goto-char (point-max))
          (unless (bolp)
            (newline))
          (org-paste-subtree 1 nil nil t))
      (org-back-to-heading t)
      (let ((level (org-get-valid-level (funcall outline-level) 1))
            (reversed (org-notes-order-reversed-p)))
        (goto-char
         (if reversed
             (or (outline-next-heading) (point-max))
           (or (save-excursion (org-get-next-sibling))
               (org-end-of-subtree t t)
               (point-max))))
        (unless (bolp)
          (newline))
        (org-paste-subtree level nil nil t))))
  (when org-auto-align-tags
    (let ((org-loop-over-headlines-in-active-region nil))
      (org-align-tags))))

(defun org-slipbox-demote-entire-buffer ()
  "Convert the current file note into a single top-level heading node."
  (interactive)
  (org-with-point-at 1
    (let ((title (org-slipbox--current-file-title))
          (tags (org-slipbox--file-tags)))
      (org-map-region #'org-do-demote (point-min) (point-max))
      (insert "* " title "\n")
      (org-back-to-heading)
      (when tags
        (org-set-tags tags))
      (org-slipbox--set-file-keyword "TITLE" nil)
      (org-slipbox--set-file-keyword "FILETAGS" nil))))

(defun org-slipbox--h1-count ()
  "Count level-1 headings in the current file."
  (let ((count 0))
    (org-with-wide-buffer
     (org-map-region
      (lambda ()
        (when (= (org-current-level) 1)
          (setq count (1+ count))))
      (point-min)
      (point-max)))
    count))

(defun org-slipbox--buffer-promoteable-p ()
  "Return non-nil when the current buffer can become a file node."
  (and (= (org-slipbox--h1-count) 1)
       (org-with-point-at 1 (org-at-heading-p))))

(defun org-slipbox-promote-entire-buffer ()
  "Convert a single level-1 heading buffer into a file node and sync it."
  (interactive)
  (org-slipbox--promote-entire-buffer-internal)
  (org-slipbox--save-and-sync-current-buffer))

(defun org-slipbox--promote-entire-buffer-internal ()
  "Convert the current single level-1 heading buffer into a file node."
  (unless (org-slipbox--buffer-promoteable-p)
    (user-error "Cannot promote: multiple root headings or extra file-level text"))
  (org-with-point-at 1
    (let ((title (nth 4 (org-heading-components)))
          (tags (org-get-tags)))
      (org-fold-show-all)
      (kill-whole-line)
      (org-slipbox--set-file-keyword "TITLE" title)
      (when tags
        (org-slipbox--set-file-tags tags))
      (org-map-region #'org-promote (point-min) (point-max)))))

(defun org-slipbox--extract-target-path (title &optional file-path)
  "Return an absolute extraction path for TITLE and FILE-PATH."
  (let* ((suggested (org-slipbox--expand-capture-template
                     org-slipbox-extract-file-name-template
                     title
                     (current-time)))
         (path (or file-path
                   (read-file-name
                    "Extract node to: "
                    (file-name-as-directory org-slipbox-directory)
                    suggested
                    nil
                    suggested)))
         (absolute (expand-file-name path org-slipbox-directory)))
    (unless (file-in-directory-p absolute (expand-file-name org-slipbox-directory))
      (user-error "%s is not inside org-slipbox-directory" absolute))
    (unless (string-suffix-p ".org" absolute)
      (user-error "Extracted file must end with .org"))
    absolute))

(defun org-slipbox--current-file-title ()
  "Return the current file node title."
  (or (org-slipbox--file-keyword-value "TITLE")
      (and buffer-file-name (file-name-base buffer-file-name))
      "note"))

(defun org-slipbox--buffer-empty-p (buffer)
  "Return non-nil when BUFFER has no meaningful content."
  (with-current-buffer buffer
    (string-empty-p
     (string-trim
      (buffer-substring-no-properties (point-min) (point-max))))))

(defun org-slipbox--sync-node-buffer-if-needed ()
  "Save and sync the current node buffer when it has local modifications."
  (let ((base-buffer (or (buffer-base-buffer) (current-buffer))))
    (when (and (org-slipbox--queryable-node-buffer-p)
               (buffer-modified-p base-buffer))
      (with-current-buffer base-buffer
        (org-slipbox--save-and-sync-current-buffer)))))

(defun org-slipbox--queryable-node-buffer-p ()
  "Return non-nil when the current buffer can resolve indexed nodes."
  (let ((file (org-slipbox--current-node-buffer-file)))
    (and file
         org-slipbox-directory
         (string-suffix-p ".org" file)
         (file-in-directory-p (expand-file-name file)
                              (expand-file-name org-slipbox-directory)))))

(defun org-slipbox--current-node-buffer-file ()
  "Return the current base buffer file path."
  (buffer-file-name (or (buffer-base-buffer) (current-buffer))))

(defun org-slipbox--replace-slipbox-links-on-save-h ()
  "Replace title-based org-slipbox links before saving when configured."
  (when org-slipbox-link-auto-replace
    (org-slipbox-link-replace-all)))

(defun org-slipbox--read-existing-node (prompt)
  "Read and return an existing node using PROMPT."
  (let* ((query (read-string prompt))
         (exact (and (not (string-empty-p query))
                     (condition-case nil
                         (org-slipbox-node-from-title-or-alias query t)
                       (error nil)))))
    (or exact
        (let* ((choices (org-slipbox--search-node-choices query))
               (selection (and choices
                               (completing-read "Node: " choices nil t))))
          (unless selection
            (user-error "No nodes match %s" query))
          (cdr (assoc selection choices))))))

(defun org-slipbox--search-node-choices (query)
  "Return display-to-node choices for QUERY."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchNodes"
                    `(:query ,query :limit ,org-slipbox-search-limit)))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes))))
    (mapcar (lambda (node)
              (cons (org-slipbox--node-display node) node))
            nodes)))

(defun org-slipbox--title-completion-table (string pred action)
  "Completion table for node titles and aliases using STRING, PRED, and ACTION."
  (if (eq action 'metadata)
      '(metadata (category . org-slipbox-node))
    (complete-with-action
     action
     (org-slipbox--title-completion-candidates string)
     string
     pred)))

(defun org-slipbox--title-completion-candidates (query)
  "Return title and alias candidates matching QUERY."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchNodes"
                    `(:query ,query :limit ,org-slipbox-search-limit)))
         (nodes (org-slipbox--plist-sequence (plist-get response :nodes)))
         candidates)
    (dolist (node nodes)
      (dolist (candidate (delete-dups
                          (append (list (plist-get node :title))
                                  (org-slipbox--plist-sequence (plist-get node :aliases)))))
        (when (and (stringp candidate)
                   (or (string-empty-p query)
                       (string-prefix-p query candidate completion-ignore-case)))
          (push candidate candidates))))
    (delete-dups (nreverse candidates))))

(defun org-slipbox--tag-completion-table (string pred action)
  "Completion table for tags using STRING, PRED, and ACTION."
  (complete-with-action
   action
   (delete-dups
    (append (org-slipbox--indexed-tags string org-slipbox-tag-search-limit)
            (org-slipbox--matching-org-tags string)))
   string
   pred))

(defun org-slipbox--indexed-tags (query limit)
  "Return indexed tags matching QUERY, requesting up to LIMIT results."
  (let* ((response (org-slipbox-rpc-request
                    "slipbox/searchTags"
                    `(:query ,query :limit ,limit)))
         (tags (plist-get response :tags)))
    (org-slipbox--plist-sequence tags)))

(defun org-slipbox--matching-org-tags (&optional prefix)
  "Return `org-tag-alist' tags matching PREFIX."
  (let ((prefix (or prefix "")))
    (seq-filter
     (lambda (tag)
       (or (string-empty-p prefix)
           (string-prefix-p prefix tag completion-ignore-case)))
     (delete-dups
      (delq nil
            (mapcar (lambda (entry)
                      (pcase entry
                        (`(,tag . ,_) (and (stringp tag) tag))
                        (_ nil)))
                    org-tag-alist))))))

(defun org-slipbox--current-node-tags ()
  "Return local tags for the current node."
  (save-excursion
    (save-restriction
      (widen)
      (if (org-slipbox--file-node-p)
          (org-slipbox--file-tags)
        (org-back-to-heading t)
        (org-get-tags nil t)))))

(defun org-slipbox--set-current-node-tags (tags)
  "Set current node tags to TAGS."
  (save-excursion
    (save-restriction
      (widen)
      (if (org-slipbox--file-node-p)
          (org-slipbox--set-file-tags tags)
        (org-back-to-heading t)
        (org-set-tags tags))))
  (org-slipbox--save-and-sync-current-buffer)
  tags)

(defun org-slipbox--file-tags ()
  "Return file-level tags from `#+FILETAGS:'."
  (let ((value (org-slipbox--file-keyword-value "FILETAGS")))
    (if value
        (org-slipbox--parse-colon-tags value)
      nil)))

(defun org-slipbox--set-file-tags (tags)
  "Set file-level TAGS in `#+FILETAGS:'."
  (org-slipbox--set-file-keyword
   "FILETAGS"
   (and tags (org-slipbox--format-colon-tags tags))))

(defun org-slipbox--file-keyword-value (keyword)
  "Return file-level KEYWORD value, or nil if missing."
  (save-excursion
    (goto-char (point-min))
    (let ((case-fold-search t)
          (limit (org-slipbox--file-keyword-limit)))
      (when (re-search-forward
             (format "^[ \t]*#\\+%s:[ \t]*\\(.*\\)$" (regexp-quote keyword))
             limit
             t)
        (string-trim (match-string 1))))))

(defun org-slipbox--set-file-keyword (keyword value)
  "Set file-level KEYWORD to VALUE.
When VALUE is nil, remove KEYWORD."
  (save-excursion
    (goto-char (point-min))
    (let ((case-fold-search t)
          (limit (org-slipbox--file-keyword-limit)))
      (if (re-search-forward
           (format "^[ \t]*#\\+%s:[ \t]*\\(.*\\)$" (regexp-quote keyword))
           limit
           t)
          (if value
              (replace-match (format "#+%s: %s" keyword value) t t)
            (delete-region (line-beginning-position)
                           (min (point-max) (1+ (line-end-position)))))
        (when value
          (goto-char (org-slipbox--file-property-insert-point))
          (insert (format "#+%s: %s\n" keyword value)))))))

(defun org-slipbox--file-keyword-limit ()
  "Return the buffer position where file-level keywords stop."
  (save-excursion
    (goto-char (point-min))
    (if (re-search-forward org-outline-regexp-bol nil t)
        (line-beginning-position)
      (point-max))))

(defun org-slipbox--visit-node (node)
  "Visit NODE in its source file."
  (find-file (expand-file-name (plist-get node :file_path) org-slipbox-directory))
  (goto-char (point-min))
  (forward-line (1- (plist-get node :line))))

(defun org-slipbox--parse-colon-tags (value)
  "Parse VALUE from an Org colon tag string."
  (let ((trimmed (string-trim (or value ""))))
    (if (and (string-prefix-p ":" trimmed)
             (string-suffix-p ":" trimmed))
        (split-string trimmed ":" t)
      nil)))

(defun org-slipbox--format-colon-tags (tags)
  "Format TAGS as an Org colon tag string."
  (format ":%s:" (string-join tags ":")))

(provide 'org-slipbox-node)

;;; org-slipbox-node.el ends here
