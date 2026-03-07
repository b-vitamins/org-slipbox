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

(defcustom org-slipbox-capture-templates
  '(("d" "default" :path "${slug}.org" :title "${title}"))
  "Capture templates for `org-slipbox-capture'.
Each template is a list of the form (KEY DESCRIPTION [:path STRING] [:title STRING])."
  :type 'sexp
  :group 'org-slipbox)

(defun org-slipbox-index ()
  "Rebuild the local org-slipbox index from Org files."
  (interactive)
  (let* ((response (org-slipbox-rpc-request "slipbox/index"))
         (files (plist-get response :files_indexed))
         (nodes (plist-get response :nodes_indexed))
         (links (plist-get response :links_indexed)))
    (message "Indexed %s files, %s nodes, %s links" files nodes links)
    response))

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
  (save-buffer)
  (org-slipbox-rpc-request
   "slipbox/indexFile"
   `(:file_path ,(expand-file-name buffer-file-name))))

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
