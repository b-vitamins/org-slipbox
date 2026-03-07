;;; test-org-slipbox.el --- Tests for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;;; Commentary:

;; Basic smoke tests for the Elisp package.

;;; Code:

(require 'cl-lib)
(require 'ert)
(require 'org-slipbox)

(defun org-slipbox-test--write-literal-file (file &optional content)
  "Write CONTENT to FILE without invoking special file-name handlers."
  (let (file-name-handler-alist)
    (write-region (or content "") nil file nil 'silent)))

(ert-deftest org-slipbox-test-feature-provided ()
  "The package entry feature should load cleanly."
  (should (featurep 'org-slipbox)))

(ert-deftest org-slipbox-test-export-mode-preserves-id-html-targets ()
  "Optional export support should keep HTML IDs aligned with `id:' links."
  (require 'org-slipbox-export)
  (let ((org-slipbox-export-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-export-mode 1)
          (let ((html (org-export-string-as
                       "* Heading\n:PROPERTIES:\n:ID: heading-id\n:END:\nSee [[id:heading-id][Jump]].\n"
                       'html t)))
            (should (string-match-p "href=\"#ID-heading-id\"" html))
            (should (string-match-p "id=\"ID-heading-id\"" html))))
      (org-slipbox-export-mode -1))))

(ert-deftest org-slipbox-test-package-load-has-no-global-side-effects ()
  "Loading the package should not install global hooks."
  (require 'calendar)
  (require 'org-id)
  (require 'org-protocol)
  (should-not (memq #'org-slipbox-sync-current-buffer after-save-hook))
  (should-not (memq #'org-slipbox--autosync-setup-file-h find-file-hook))
  (should-not (advice-member-p #'org-slipbox--autosync-rename-file-a 'rename-file))
  (should-not (advice-member-p #'org-slipbox--autosync-delete-file-a 'delete-file))
  (should-not (advice-member-p #'org-slipbox--autosync-vc-delete-file-a 'vc-delete-file))
  (should-not (advice-member-p #'org-slipbox-id-find 'org-id-find))
  (should-not (memq #'org-slipbox-buffer--redisplay-h post-command-hook))
  (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                    calendar-today-visible-hook))
  (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                    calendar-today-invisible-hook))
  (should-not (assoc "org-slipbox-ref" org-protocol-protocol-alist))
  (should-not (assoc "org-slipbox-node" org-protocol-protocol-alist)))

(ert-deftest org-slipbox-test-id-mode-toggles-org-id-find-advice ()
  "The org-id bridge mode should own its advice explicitly."
  (require 'org-id)
  (let ((org-slipbox-id-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-id-mode 1)
          (should (advice-member-p #'org-slipbox-id-find 'org-id-find))
          (org-slipbox-id-mode -1)
          (should-not (advice-member-p #'org-slipbox-id-find 'org-id-find)))
      (when org-slipbox-id-mode
        (org-slipbox-id-mode -1)))))

(ert-deftest org-slipbox-test-org-id-find-prefers-indexed-location-over-stale-org-id-cache ()
  "The org-id bridge should let indexed truth win over stale org-id-locations."
  (require 'org-id)
  (let* ((root (make-temp-file "org-slipbox-id-" t))
         (note (expand-file-name "note.org" root))
         (stale (expand-file-name "stale.org" root))
         (org-directory nil)
         (org-id-track-globally t)
         (org-id-locations (make-hash-table :test 'equal))
         (org-id-files nil)
         (org-id-locations-file (expand-file-name ".org-id-locations" root))
         (org-id--locations-checksum nil))
    (unwind-protect
        (progn
          (write-region "* Heading\n:PROPERTIES:\n:ID: note-id\n:END:\n" nil note nil 'silent)
          (write-region "* Stale\n" nil stale nil 'silent)
          (puthash "note-id" (abbreviate-file-name stale) org-id-locations)
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-node-from-id)
                       (lambda (id)
                         (and (string= id "note-id")
                              '(:file_path "note.org" :line 1)))))
              (unwind-protect
                  (progn
                    (org-slipbox-id-mode 1)
                    (let ((location (org-id-find "note-id")))
                      (should (equal (car location) note))
                      (with-temp-buffer
                        (insert-file-contents note)
                        (goto-char (cdr location))
                        (should (= (line-number-at-pos) 1)))))
                (when org-slipbox-id-mode
                  (org-slipbox-id-mode -1))))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-org-id-find-falls-back-to-org-id-locations-for-excluded-files ()
  "The org-id bridge should preserve valid excluded ID targets via fallback."
  (require 'org-id)
  (let* ((root (make-temp-file "org-slipbox-id-" t))
         (archive (expand-file-name "archive" root))
         (excluded (expand-file-name "excluded.org" archive))
         (extra-dir (make-temp-file "org-slipbox-extra-id-" t))
         (extra-file (expand-file-name "extra.org" extra-dir))
         (org-directory nil)
         (org-id-track-globally t)
         (org-id-locations nil)
         (org-id-files nil)
         (org-id-locations-file (expand-file-name ".org-id-locations" root))
         (org-id-extra-files nil)
         (org-agenda-files nil)
         (org-id--locations-checksum nil))
    (unwind-protect
        (progn
          (make-directory archive t)
          (write-region "* Excluded\n:PROPERTIES:\n:ID: excluded-id\n:END:\n"
                        nil excluded nil 'silent)
          (write-region "* Extra\n:PROPERTIES:\n:ID: extra-id\n:END:\n"
                        nil extra-file nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-file-exclude-regexp "^archive/"))
            (org-slipbox-update-org-id-locations extra-dir)
            (should (equal (gethash "excluded-id" org-id-locations)
                           (abbreviate-file-name excluded)))
            (should (equal (gethash "extra-id" org-id-locations)
                           (abbreviate-file-name extra-file)))
            (cl-letf (((symbol-function 'org-slipbox-node-from-id) (lambda (_id) nil)))
              (unwind-protect
                  (progn
                    (org-slipbox-id-mode 1)
                    (let ((location (org-id-find "excluded-id")))
                      (should (equal (car location) (abbreviate-file-name excluded)))
                      (with-temp-buffer
                        (insert-file-contents excluded)
                        (goto-char (cdr location))
                        (should (= (line-number-at-pos) 1)))))
                (when org-slipbox-id-mode
                  (org-slipbox-id-mode -1))))))
      (delete-directory root t)
      (delete-directory extra-dir t))))

(ert-deftest org-slipbox-test-file-p-respects-discovery-policy ()
  "File eligibility should honor extensions, encrypted suffixes, and exclusions."
  (let* ((root (make-temp-file "org-slipbox-files-" t))
         (archive (expand-file-name "archive" root))
         (plain (expand-file-name "note.org" root))
         (gpg (expand-file-name "secret.org.gpg" root))
         (age (expand-file-name "locked.org.age" root))
         (archived (expand-file-name "skip.org" archive))
         (markdown (expand-file-name "readme.md" root))
         (outside-root (make-temp-file "org-slipbox-outside-" t))
         (outside (expand-file-name "outside.org" outside-root)))
    (unwind-protect
        (progn
          (make-directory archive t)
          (write-region "" nil plain nil 'silent)
          (org-slipbox-test--write-literal-file gpg)
          (org-slipbox-test--write-literal-file age)
          (write-region "" nil archived nil 'silent)
          (write-region "" nil markdown nil 'silent)
          (write-region "" nil outside nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-file-extensions '("org"))
                (org-slipbox-file-exclude-regexp "^archive/"))
            (should (org-slipbox-file-p plain))
            (should (org-slipbox-file-p gpg))
            (should (org-slipbox-file-p age))
            (should-not (org-slipbox-file-p archived))
            (should-not (org-slipbox-file-p markdown))
            (should-not (org-slipbox-file-p outside))))
      (delete-directory root t)
      (delete-directory outside-root t))))

(ert-deftest org-slipbox-test-list-files-respects-discovery-policy ()
  "File listing should use relative exclusions and configured extensions."
  (let* ((root (make-temp-file "org-slipbox-files-root-" t))
         (archive (expand-file-name "archive" root))
         (basename-regexp (regexp-quote (file-name-nondirectory root)))
         (org-file (expand-file-name "note.org" root))
         (md-file (expand-file-name "readme.md" root))
         (gpg-file (expand-file-name "secret.md.gpg" root))
         (archived (expand-file-name "skip.md" archive)))
    (unwind-protect
        (progn
          (make-directory archive t)
          (write-region "" nil org-file nil 'silent)
          (write-region "" nil md-file nil 'silent)
          (org-slipbox-test--write-literal-file gpg-file)
          (write-region "" nil archived nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-file-extensions '("org" ".md"))
                (org-slipbox-file-exclude-regexp (list "^archive/" basename-regexp)))
            (should
             (equal
              (mapcar #'file-name-nondirectory (org-slipbox-list-files))
              '("note.org" "readme.md" "secret.md.gpg")))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-rpc-command-includes-discovery-policy ()
  "Daemon startup should include the configured discovery policy."
  (let ((org-slipbox-server-program "/tmp/slipbox")
        (org-slipbox-directory "/tmp/notes")
        (org-slipbox-database-file "/tmp/org-slipbox.sqlite")
        (org-slipbox-file-extensions '("org" ".md"))
        (org-slipbox-file-exclude-regexp '("^archive/" "\\.cache/")))
    (should
     (equal
      (org-slipbox-rpc--command)
      '("/tmp/slipbox"
        "serve"
        "--root" "/tmp/notes"
        "--db" "/tmp/org-slipbox.sqlite"
        "--file-extension" "org"
        "--file-extension" "md"
        "--exclude-regexp" "^archive/"
        "--exclude-regexp" "\\.cache/")))))

(ert-deftest org-slipbox-test-node-display-includes-file-and-line ()
  "Node display strings should be stable and informative."
  (should
   (equal
    (org-slipbox--node-display
     '(:title "Heading" :outline_path "Parent / Heading" :file_path "notes/foo.org" :line 42))
    "Heading | Parent / Heading | notes/foo.org:42")))

(ert-deftest org-slipbox-test-node-display-omits-empty-outline ()
  "Display strings should not emit empty outline segments."
  (should
   (equal
    (org-slipbox--node-display
     '(:title "Heading" :outline_path "" :file_path "notes/foo.org" :line 42))
    "Heading | notes/foo.org:42")))

(ert-deftest org-slipbox-test-node-display-includes-tags ()
  "Display strings should surface node tags."
  (should
   (equal
    (org-slipbox--node-display
     '(:title "Heading" :outline_path "" :tags ["one" "two"] :file_path "notes/foo.org" :line 42))
    "Heading | #one #two | notes/foo.org:42")))

(ert-deftest org-slipbox-test-node-candidate-display-supports-string-template ()
  "Node candidate templates should interpolate supported fields."
  (should
   (equal
    (org-slipbox--format-node-template
     "${title:*} ${tags:8} ${file}"
     '(:title "Heading"
       :tags ["one" "two"]
       :file_path "notes/foo.org")
     30)
    "Heading #one #tw notes/foo.org")))

(ert-deftest org-slipbox-test-search-node-choices-use-configured-display-template ()
  "Interactive node choices should use the configured candidate formatter."
  (let ((org-slipbox-node-display-template
         (lambda (node)
           (format "Choice: %s" (plist-get node :title)))))
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit)
                 '(:nodes [(:title "One" :file_path "one.org" :line 1)]))))
      (let ((choices (org-slipbox--search-node-choices "o")))
        (should (equal (mapcar #'substring-no-properties (mapcar #'car choices))
                       '("Choice: One")))
        (should (equal (mapcar #'cdr choices)
                       '((:title "One" :file_path "one.org" :line 1))))))))

(ert-deftest org-slipbox-test-node-read-returns-new-title-when-no-match ()
  "Node read should return a new-title placeholder when no match is required."
  (cl-letf (((symbol-function 'org-slipbox-node-read--completions)
             (lambda (&rest _args) nil))
            ((symbol-function 'completing-read)
             (lambda (&rest _args)
               "Fresh Node")))
    (should (equal (org-slipbox-node-read) '(:title "Fresh Node")))))

(ert-deftest org-slipbox-test-node-read-applies-default-sort-and-annotation ()
  "Node read should expose sort and annotation metadata."
  (let ((org-slipbox-node-default-sort 'title)
        (org-slipbox-node-annotation-function
         (lambda (node)
           (format " [%s]" (plist-get node :file_path))))
        metadata
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit)
                 '(:nodes [(:title "Zulu" :file_path "z.org" :line 2)
                           (:title "Alpha" :file_path "a.org" :line 1)])))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil)
                       metadata (funcall collection "" nil 'metadata))
                 (car candidates))))
      (let* ((node (org-slipbox-node-read))
             (props (cdr metadata))
             (annotation (funcall (cdr (assq 'annotation-function props))
                                  (car candidates))))
        (should (equal (plist-get node :title) "Alpha"))
        (should (eq (cdr (assq 'display-sort-function props)) 'identity))
        (should (equal annotation " [a.org]"))))))

(ert-deftest org-slipbox-test-node-insert-uses-node-formatter ()
  "Node insertion should honor `org-slipbox-node-formatter'."
  (with-temp-buffer
    (let ((org-slipbox-node-formatter "${title} ${todo} ${tags}"))
      (cl-letf (((symbol-function 'org-slipbox-node-read)
                 (lambda (&rest _args)
                   '(:title "Heading"
                     :file_path "notes/heading.org"
                     :node_key "file:notes/heading.org"
                     :line 1
                     :tags ["one"]
                     :todo_keyword "TODO")))
                ((symbol-function 'org-slipbox--ensure-node-id)
                 (lambda (node)
                   (plist-put (copy-sequence node) :explicit_id "node-1"))))
        (org-slipbox-node-insert)
        (should (equal (buffer-string)
                       "[[id:node-1][Heading t:TODO #one]]"))))))

(ert-deftest org-slipbox-test-node-insert-replaces-active-region ()
  "Node insertion should use the active region as the link description."
  (with-temp-buffer
    (org-mode)
    (transient-mark-mode 1)
    (insert "Selected text")
    (goto-char (point-min))
    (set-mark (point-max))
    (activate-mark)
    (cl-letf (((symbol-function 'org-slipbox-node-read)
               (lambda (&rest _args)
                 '(:title "Heading"
                   :file_path "notes/heading.org"
                   :node_key "file:notes/heading.org"
                   :line 1)))
              ((symbol-function 'org-slipbox--ensure-node-id)
               (lambda (node)
                 (plist-put (copy-sequence node) :explicit_id "node-1"))))
      (org-slipbox-node-insert)
      (should (equal (buffer-string)
                     "[[id:node-1][Selected text]]")))))

(ert-deftest org-slipbox-test-node-insert-captures-and-replaces-active-region ()
  "Region-aware insertion should keep the selected text when capturing a node."
  (with-temp-buffer
    (org-mode)
    (transient-mark-mode 1)
    (insert "Fresh title")
    (goto-char (point-min))
    (set-mark (point-max))
    (activate-mark)
    (cl-letf (((symbol-function 'org-slipbox-node-read)
               (lambda (&rest _args)
                 '(:title "Fresh title")))
              ((symbol-function 'org-slipbox--capture-node)
               (lambda (_title &optional _template _refs _variables session)
                 (let ((node '(:title "Fresh title"
                               :file_path "fresh.org"
                               :node_key "file:fresh.org"
                               :line 1
                               :explicit_id "fresh-1")))
                   (org-slipbox--capture-finalize-insert-link node session)
                   node))))
      (org-slipbox-node-insert)
      (should (equal (buffer-string)
                     "[[id:fresh-1][Fresh title]]")))))

(ert-deftest org-slipbox-test-capture-template-expansion ()
  "Capture templates should expand built-in and contextual placeholders."
  (should
   (equal
    (org-slipbox--expand-capture-template
     "notes/%<%Y>-${slug}.org"
     "Sample Title"
     (encode-time 0 0 0 7 3 2026))
    "notes/2026-sample-title.org"))
  (should
   (equal
    (org-slipbox--expand-capture-template
     "Slip: ${title}"
     "Sample Title"
     (encode-time 0 0 0 7 3 2026))
    "Slip: Sample Title"))
  (should
   (equal
    (org-slipbox--expand-capture-template
     "Source ${ref}\n${body}"
     "Sample Title"
     (encode-time 0 0 0 7 3 2026)
     '(:ref "https://example.test/article"
       :body "Quoted text"))
    "Source https://example.test/article\nQuoted text")))

(ert-deftest org-slipbox-test-capture-node-opens-shorthand-draft ()
  "Capture should open a transient shorthand draft before writing."
  (let (buffer)
    (unwind-protect
        (progn
          (setq buffer
                (org-slipbox--capture-node
                 "Sample Title"
                 '("d" "default" :path "notes/%<%Y>-${slug}.org" :title "Slip: ${title}")))
          (should (buffer-live-p buffer))
          (with-current-buffer buffer
            (should (derived-mode-p 'org-slipbox-capture-mode))
            (should (org-slipbox-capture-session-p org-slipbox-capture--session))
            (should
             (equal
              (org-slipbox-capture-session-capture-title org-slipbox-capture--session)
              "Slip: Sample Title"))
            (should
             (equal
              (org-slipbox-capture-session-target org-slipbox-capture--session)
              '(:kind file :file_path "notes/2026-sample-title.org")))
            (should
             (equal
              (buffer-substring-no-properties org-slipbox--capture-body-start (point-max))
              ""))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-finalize-commits-shorthand-draft ()
  "Finalizing a shorthand draft should write through the generic capture RPC."
  (let (buffer method params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Slip: Sample Title"
                         :file_path "notes/2026-sample-title.org"
                         :line 1))))
            (setq buffer
                  (org-slipbox--capture-node
                   "Sample Title"
                   '("d" "default"
                     :path "notes/%<%Y>-${slug}.org"
                     :title "Slip: ${title}")))
            (with-current-buffer buffer
              (goto-char (point-max))
              (insert "Body text")
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p buffer))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Slip: Sample Title"
                    :capture_type "plain"
                    :content "Body text"
                    :prepend nil
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :file_path "notes/2026-sample-title.org"))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-finalize-commits-outline-path-draft ()
  "Outline-path shorthand drafts should resolve to entry capture params."
  (let (buffer method params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 8)))
                    ((symbol-function 'current-time)
                     (lambda ()
                       (encode-time 0 0 0 7 3 2026))))
            (setq buffer
                  (org-slipbox--capture-node
                   "Meeting"
                   '("d" "daily"
                     :target (file+head+olp
                              "daily/%<%Y-%m-%d>.org"
                              "#+title: %<%Y-%m-%d>"
                              ("Inbox")))))
            (with-current-buffer buffer
              (goto-char (point-max))
              (insert "Discuss roadmap")
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p buffer))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Meeting"
                    :capture_type "entry"
                    :content "Discuss roadmap"
                    :prepend nil
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :file_path "daily/2026-03-07.org"
                    :head "#+title: 2026-03-07"
                    :outline_path ("Inbox")))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-node-shows-file-head-preview ()
  "Draft buffers should show target metadata previews without mutating files."
  (let (buffer)
    (unwind-protect
        (progn
          (setq buffer
                (org-slipbox--capture-node
                 "Note"
                 '("d" "default"
                   :target (file+head
                            "notes/${slug}.org"
                            "#+title: ${title}\n#+filetags: :seed:"))))
          (with-current-buffer buffer
            (should (string-match-p "^# Target: notes/note.org$" (buffer-string)))
            (should (string-match-p "^# Preview:$" (buffer-string)))
            (should (string-match-p "^#   #\\+title: Note$" (buffer-string)))
            (should (string-match-p "^#   #\\+filetags: :seed:$" (buffer-string)))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-ref-reuses-existing-node ()
  "Ref capture should reuse an existing indexed ref node."
  (let (visited)
    (cl-letf (((symbol-function 'org-slipbox-node-from-ref)
               (lambda (_reference)
                 '(:title "Existing" :file_path "existing.org" :line 2)))
              ((symbol-function 'org-slipbox--capture-node)
               (lambda (&rest _args)
                 (ert-fail "capture should not run when ref already exists")))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional _other-window)
                 (setq visited node))))
      (org-slipbox-capture-ref "@smith2024"))
    (should
     (equal visited
            '(:title "Existing" :file_path "existing.org" :line 2)))))

(ert-deftest org-slipbox-test-capture-ref-creates-node-with-ref ()
  "Ref capture should pass the ref through the capture pipeline when missing."
  (let (captured visited)
    (cl-letf (((symbol-function 'org-slipbox-node-from-ref)
              (lambda (_reference) nil))
              ((symbol-function 'org-slipbox--read-capture-template)
               (lambda (&rest _args)
                 '("d" "default" :path "notes/${slug}.org" :title "${title}")))
              ((symbol-function 'org-slipbox--capture-node)
               (lambda (title template refs variables &optional session)
                 (setq captured (list title template refs variables session))
                 (let ((node '(:title "New" :file_path "new.org" :line 1)))
                   (org-slipbox--visit-node node)
                   node)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional _other-window)
                 (setq visited node))))
      (org-slipbox-capture-ref "cite:smith2024" "New"))
    (should
     (equal captured
            '("New"
              ("d" "default" :path "notes/${slug}.org" :title "${title}")
              ("cite:smith2024")
              (:ref "cite:smith2024")
              (:default-finalize find-file))))
    (should
     (equal visited
            '(:title "New" :file_path "new.org" :line 1)))))

(ert-deftest org-slipbox-test-capture-finalize-insert-link-runs-hook ()
  "Insert-link finalization should replace the region and run the insert hook."
  (with-temp-buffer
    (org-mode)
    (transient-mark-mode 1)
    (insert "Selected text")
    (goto-char (point-min))
    (set-mark (point-max))
    (activate-mark)
    (let ((region (cons (copy-marker (region-beginning))
                        (copy-marker (region-end))))
          (marker (point-marker))
          hook-args)
      (goto-char (point-min))
      (cl-letf (((symbol-function 'org-slipbox--ensure-node-id)
                 (lambda (node)
                   node))
                ((symbol-function 'run-hook-with-args)
                 (lambda (_hook id description)
                   (setq hook-args (list id description)))))
        (org-slipbox--capture-finalize-insert-link
         '(:title "Note"
           :file_path "note.org"
           :line 1
           :explicit_id "note-1")
         `(:call-location ,marker
           :region ,region
           :link-description "Selected text")))
      (should (equal (buffer-string)
                     "[[id:note-1][Selected text]]"))
      (should (equal hook-args '("note-1" "Selected text"))))))

(ert-deftest org-slipbox-test-capture-to-node-uses-rpc ()
  "Node-target capture should use the dedicated append-to-node RPC."
  (let (method params visited)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Child" :file_path "project.org" :line 8)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional _other-window)
                 (setq visited node))))
      (org-slipbox-capture-to-node
       '(:node_key "heading:project.org:3" :title "Parent")
       "Child"))
    (should (equal method "slipbox/appendHeadingToNode"))
    (should
     (equal params
            '(:node_key "heading:project.org:3" :heading "Child")))
    (should
     (equal visited
            '(:title "Child" :file_path "project.org" :line 8)))))

(ert-deftest org-slipbox-test-capture-finalize-commits-typed-draft ()
  "Typed templates should render into an editable draft and commit through RPC."
  (let (buffer method params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Note" :file_path "notes/note.org" :line 5))))
            (setq buffer
                  (org-slipbox--capture-node-at-time
                   "Note"
                   '("d" "default" entry "* ${title}\n${body}"
                     :target (file+head+olp
                              "notes/${slug}.org"
                              "#+title: ${title}"
                              ("Inbox"))
                     :prepend t
                     :empty-lines 1)
                   '("https://example.test/ref")
                   (encode-time 0 0 0 7 3 2026)
                   '(:body "Body text")))
            (with-current-buffer buffer
              (should
               (equal
                (buffer-substring-no-properties org-slipbox--capture-body-start (point-max))
                "* Note\nBody text"))
              (goto-char (point-max))
              (insert "\nMore")
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p buffer))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Note"
                    :capture_type "entry"
                    :content "* Note\nBody text\nMore"
                    :prepend t
                    :empty_lines_before 1
                    :empty_lines_after 1
                    :refs ("https://example.test/ref")
                    :file_path "notes/note.org"
                    :head "#+title: Note"
                    :outline_path ("Inbox")))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-immediate-finish-commits-without-draft ()
  "Immediate-finish capture should commit directly without displaying a draft."
  (let (method params result)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Note" :file_path "notes/note.org" :line 5))))
      (setq result
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" plain "${title}"
               :target (file "notes/${slug}.org")
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
      (should-not (get-buffer "*org-slipbox capture: Note*"))
      (should (equal result '(:title "Note" :file_path "notes/note.org" :line 5)))
      (should (equal method "slipbox/captureTemplate"))
      (should
       (equal params
              '(:title "Note"
                :capture_type "plain"
                :content "Note"
                :prepend nil
                :empty_lines_before 0
                :empty_lines_after 0
                :file_path "notes/note.org"))))))

(ert-deftest org-slipbox-test-capture-immediate-finish-insert-link ()
  "Immediate-finish capture should support insert-link finalization."
  (with-temp-buffer
    (org-mode)
    (let ((marker (point-marker)))
      (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                 (lambda (&rest _args)
                   '(:title "Note"
                     :file_path "notes/note.org"
                     :line 5
                     :explicit_id "note-1")))
                ((symbol-function 'org-slipbox--ensure-node-id)
                 (lambda (node)
                   node)))
        (org-slipbox--capture-node-at-time
         "Note"
         '("d" "default" plain "${title}"
           :target (file "notes/${slug}.org")
           :immediate-finish t)
         nil
         (encode-time 0 0 0 7 3 2026)
         nil
         `(:finalize insert-link
           :call-location ,marker
           :link-description "Inserted"))
        (should (equal (buffer-string)
                       "[[id:note-1][Inserted]]"))))))

(ert-deftest org-slipbox-test-capture-immediate-finish-jumps-to-captured-node ()
  "Immediate-finish capture should still honor `:jump-to-captured'."
  (let (visited)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (&rest _args)
                 '(:title "Note" :file_path "notes/note.org" :line 5)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional _other-window)
                 (setq visited node))))
      (org-slipbox--capture-node-at-time
       "Note"
       '("d" "default" plain "${title}"
         :target (file "notes/${slug}.org")
         :jump-to-captured t
         :immediate-finish t)
       nil
       (encode-time 0 0 0 7 3 2026))
      (should (equal visited '(:title "Note" :file_path "notes/note.org" :line 5))))))

(ert-deftest org-slipbox-test-capture-lifecycle-handlers-run-in-order ()
  "Lifecycle handlers should run in org-capture order with dynamic context."
  (let (events)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (&rest _args)
                 '(:title "Note" :file_path "notes/note.org" :line 5))))
      (org-slipbox--capture-node-at-time
       "Note"
       `("d" "default" plain "${title}"
         :target (file "notes/${slug}.org")
         :immediate-finish t
         :prepare-finalize
         ,(lambda ()
            (push (list :prepare
                        (org-slipbox-capture-session-p org-slipbox-capture-current-session)
                        org-slipbox-capture-current-node)
                  events))
         :before-finalize
         ,(lambda ()
            (push (list :before
                        (plist-get org-slipbox-capture-current-node :title))
                  events))
         :after-finalize
         ,(lambda ()
            (push (list :after
                        (plist-get org-slipbox-capture-current-node :title))
                  events)))
       nil
       (encode-time 0 0 0 7 3 2026)))
    (setq events (nreverse events))
    (should (equal (mapcar #'car events) '(:prepare :before :after)))
    (should (equal (nth 1 (car events)) t))
    (should-not (nth 2 (car events)))
    (should (equal (cadr (nth 1 events)) "Note"))
    (should (equal (cadr (nth 2 events)) "Note"))))

(ert-deftest org-slipbox-test-capture-finalize-passes-table-line-position ()
  "Table-line drafts should pass `:table-line-pos' through to the RPC."
  (let (buffer method params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Note" :file_path "notes/note.org" :line 5))))
            (setq buffer
                  (org-slipbox--capture-node-at-time
                   "Note"
                   '("t" "table row" table-line "| ${title} |"
                     :target (file "notes/${slug}.org")
                     :table-line-pos "I+1")
                   nil
                   (encode-time 0 0 0 7 3 2026)))
            (with-current-buffer buffer
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p buffer))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Note"
                    :capture_type "table-line"
                    :content "| Note |"
                    :prepend nil
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :table_line_pos "I+1"
                    :file_path "notes/note.org"))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-typed-capture-target-node-resolves-existing-node ()
  "Typed templates should resolve `(node ...)' targets before draft creation."
  (let (buffer method params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-node-from-id)
                     (lambda (_id) nil))
                    ((symbol-function 'org-slipbox-node-from-title-or-alias)
                     (lambda (query _nocase)
                       (should (equal query "Parent"))
                       '(:node_key "heading:project.org:3" :title "Parent")))
                    ((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Parent" :file_path "project.org" :line 3))))
            (setq buffer
                  (org-slipbox--capture-node-at-time
                   "Follow up"
                   '("n" "node item" item "${title}"
                     :target (node "Parent"))
                   nil
                   (encode-time 0 0 0 7 3 2026)))
            (with-current-buffer buffer
              (should
               (equal
                (plist-get
                 (org-slipbox-capture-session-target org-slipbox-capture--session)
                 :node_key)
                "heading:project.org:3"))
              (org-slipbox-capture-finalize)))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Follow up"
                    :capture_type "item"
                    :content "Follow up"
                    :prepend nil
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :node_key "heading:project.org:3"))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-typed-capture-template-jumps-to-captured-node ()
  "Typed templates should honor `:jump-to-captured' after draft finalization."
  (let (buffer visited)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (&rest _args)
                       '(:title "Note" :file_path "notes/note.org" :line 5)))
                    ((symbol-function 'org-slipbox--visit-node)
                     (lambda (node &optional _other-window)
                       (setq visited node))))
            (setq buffer
                  (org-slipbox--capture-node-at-time
                   "Note"
                   '("d" "default" entry "* ${title}"
                     :target (file "notes/${slug}.org")
                     :jump-to-captured t)
                   nil
                   (encode-time 0 0 0 7 3 2026)))
            (with-current-buffer buffer
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p buffer))
          (should (equal visited '(:title "Note" :file_path "notes/note.org" :line 5))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-abort-skips-rpc-writes ()
  "Aborting a draft should not call any capture RPC or mutate targets."
  (let (buffer called)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (&rest _args)
                       (setq called t)
                       (ert-fail "capture RPC should not run on abort"))))
            (setq buffer
                  (org-slipbox--capture-node
                   "Abort Me"
                   '("d" "default" :path "notes/${slug}.org")))
            (with-current-buffer buffer
              (goto-char (point-max))
              (insert "Transient text")
              (org-slipbox-capture-abort)))
          (should-not called)
          (should-not (buffer-live-p buffer)))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-abort-leaves-new-file-absent ()
  "Aborting a file-target capture should leave new target files absent."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/abort-me.org" root))
         buffer)
    (unwind-protect
        (progn
          (setq buffer
                (org-slipbox--capture-node-at-time
                 "Abort Me"
                 '("d" "default" :path "notes/${slug}.org")
                 nil
                 (encode-time 0 0 0 7 3 2026)))
          (with-current-buffer buffer
            (goto-char (point-max))
            (insert "Transient text")
            (org-slipbox-capture-abort))
          (should-not (file-exists-p target)))
      (when (buffer-live-p buffer)
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-unsupported-lifecycle-option-errors ()
  "Unsupported org-capture lifecycle keys should error clearly."
  (dolist (key '(:kill-buffer :no-save :unnarrowed
                  :clock-in :clock-resume :clock-keep))
    (should-error
     (org-slipbox--capture-node
      "Note"
      `("d" "default" :path "notes/${slug}.org" ,key t))
     :type 'error)))

(ert-deftest org-slipbox-test-capture-invalid-lifecycle-handler-errors ()
  "Lifecycle handlers must be functions or lists of functions."
  (dolist (key '(:prepare-finalize :before-finalize :after-finalize))
    (should-error
     (org-slipbox--capture-node-at-time
      "Note"
      `("d" "default" plain "${title}"
        :target (file "notes/${slug}.org")
        ,key 42)
      nil
      (encode-time 0 0 0 7 3 2026))
     :type 'error)))

(ert-deftest org-slipbox-test-capture-unsupported-target-option-errors ()
  "Unsupported target-preparation keys should error clearly."
  (dolist (key '(:exact-position :insert-here))
    (should-error
     (org-slipbox--capture-node
      "Note"
      `("d" "default" :path "notes/${slug}.org" ,key t))
     :type 'error)))

(ert-deftest org-slipbox-test-capture-table-line-position-requires-table-line ()
  "Non-table captures should reject `:table-line-pos' explicitly."
  (should-error
   (org-slipbox--capture-node-at-time
    "Note"
    '("d" "default" plain "${title}"
      :target (file "notes/${slug}.org")
      :table-line-pos "I+1")
    nil
    (encode-time 0 0 0 7 3 2026))
   :type 'error))

(ert-deftest org-slipbox-test-capture-datetree-target-expands-outline-path ()
  "Datetree targets should expand to deterministic outline paths."
  (should
   (equal
    (org-slipbox--expand-capture-target
     '(:target (file+datetree "daily/%<%Y-%m>.org" week))
     "Entry"
     (encode-time 0 0 0 7 3 2026))
    '(:kind file
      :file_path "daily/2026-03.org"
      :outline_path ("2026" "2026-W10" "2026-03-07 Saturday")))))

(ert-deftest org-slipbox-test-render-capture-body-expands-org-capture-escapes ()
  "Capture body rendering should support `%'-escapes and `${...}' variables."
  (should
   (equal
    (org-slipbox--render-capture-body
     "Seen %<%Y> %i ${ref=none}"
     'plain
     "Note"
     (encode-time 0 0 0 7 3 2026)
     '(:body "Quoted text"
       :ref "cite:smith2024"))
    "Seen 2026 Quoted text cite:smith2024")))

(ert-deftest org-slipbox-test-capture-string-prompts-once-for-repeated-placeholder ()
  "Repeated `${...}' placeholders should share one prompted value."
  (let ((calls 0))
    (cl-letf (((symbol-function 'read-from-minibuffer)
               (lambda (_prompt &optional default-value)
                 (setq calls (1+ calls))
                 (or default-value "Topic"))))
      (should
       (equal
        (org-slipbox--render-capture-string
         "${topic=Topic} / ${topic=Topic}"
         "Note"
         (encode-time 0 0 0 7 3 2026))
        "Topic / Topic"))
      (should (= calls 1)))))

(ert-deftest org-slipbox-test-node-from-id-uses-rpc ()
  "Exact ID lookup should call the dedicated RPC."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Note" :file_path "note.org" :line 1))))
      (should
       (equal
        (org-slipbox-node-from-id "abc123")
        '(:title "Note" :file_path "note.org" :line 1))))
    (should (equal method "slipbox/nodeFromId"))
    (should (equal params '(:id "abc123")))))

(ert-deftest org-slipbox-test-node-from-title-or-alias-uses-rpc ()
  "Exact title or alias lookup should call the dedicated RPC."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Bruce Wayne" :file_path "wayne.org" :line 1))))
      (should
       (equal
        (org-slipbox-node-from-title-or-alias "batman" t)
        '(:title "Bruce Wayne" :file_path "wayne.org" :line 1))))
    (should (equal method "slipbox/nodeFromTitleOrAlias"))
    (should (equal params '(:title_or_alias "batman" :nocase t)))))

(ert-deftest org-slipbox-test-node-random-uses-rpc ()
  "Random node selection should use the dedicated RPC."
  (let (method params visited)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method &optional request-params)
                 (setq method request-method
                       params request-params)
                 '(:node (:title "Random" :file_path "random.org" :line 4))))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional other-window)
                 (setq visited (list node other-window)))))
      (org-slipbox-node-random t))
    (should (equal method "slipbox/randomNode"))
    (should (null params))
    (should
     (equal visited
            '((:title "Random" :file_path "random.org" :line 4) t)))))

(ert-deftest org-slipbox-test-node-at-point-syncs-modified-buffer ()
  "Point-based lookup should sync modified buffers before querying."
  (let* ((root (make-temp-file "org-slipbox-node-" t))
         (file (expand-file-name "note.org" root))
         calls)
    (unwind-protect
        (progn
          (write-region "#+title: Note\n\n* Heading\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-max))
            (insert "Body\n")
            (search-backward "* Heading")
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (request-method &optional request-params)
                           (push (list request-method request-params) calls)
                           (when (equal request-method "slipbox/nodeAtPoint")
                             '(:title "Heading" :file_path "note.org" :line 3)))))
                (should
                 (equal
                  (org-slipbox-node-at-point)
                  '(:title "Heading" :file_path "note.org" :line 3)))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            (mapcar #'car (nreverse calls))
            '("slipbox/indexFile" "slipbox/nodeAtPoint"))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-buffer-render-includes-refs-and-backlinks ()
  "Context buffer rendering should include refs and backlinks."
  (with-current-buffer (get-buffer-create "*org-slipbox test*")
    (unwind-protect
        (progn
          (setq-local org-slipbox-buffer-current-node
                      '(:node_key "heading:note.org:3"
                        :title "Heading"
                        :file_path "note.org"
                        :line 3
                        :refs ["@smith2024"]))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_method _params)
                       '(:backlinks [(:source_node (:title "Backlink"
                                              :file_path "other.org"
                                              :line 10)
                                     :row 12
                                     :col 4
                                     :preview "See [[id:heading]].")]))))
            (org-slipbox-buffer-render-contents))
          (should (derived-mode-p 'org-slipbox-buffer-mode))
          (should (string-match-p "Refs" (buffer-string)))
          (should (string-match-p "@smith2024" (buffer-string)))
          (should (string-match-p "Backlinks" (buffer-string)))
          (should (string-match-p "Backlink" (buffer-string)))
          (should (string-match-p "other.org:12:4" (buffer-string)))
          (should (string-match-p "See \\[\\[id:heading\\]\\]" (buffer-string))))
      (kill-buffer (current-buffer)))))

(ert-deftest org-slipbox-test-buffer-visit-location-moves-to-exact-position ()
  "Location visits should land on the requested row and column."
  (let* ((root (make-temp-file "org-slipbox-buffer-visit-" t))
         (org-slipbox-directory root)
         (file (expand-file-name "note.org" root)))
    (unwind-protect
        (progn
          (write-region "Line one\nSecond line\nThird line\n" nil file nil 'silent)
          (org-slipbox-buffer--visit-location "note.org" 2 3)
          (should (equal (buffer-file-name) file))
          (should (= (line-number-at-pos) 2))
          (should (= (current-column) 2))
          (kill-buffer (current-buffer)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-buffer-persistent-redisplay-renders-current-node ()
  "Persistent redisplay should adopt the node at point."
  (let (rendered)
    (cl-letf (((symbol-function 'org-slipbox-node-at-point)
               (lambda (&optional _assert)
                 '(:node_key "file:note.org"
                   :title "Note"
                   :file_path "note.org"
                   :line 1)))
              ((symbol-function 'org-slipbox-buffer-render-contents)
               (lambda ()
                 (setq rendered org-slipbox-buffer-current-node))))
      (with-current-buffer (get-buffer-create org-slipbox-buffer)
        (unwind-protect
            (progn
              (org-slipbox-buffer-persistent-redisplay)
              (should
               (equal rendered
                      '(:node_key "file:note.org"
                        :title "Note"
                        :file_path "note.org"
                        :line 1))))
          (kill-buffer (current-buffer)))))))

(ert-deftest org-slipbox-test-buffer-unlinked-rg-command-quotes-root ()
  "Unlinked-reference grep commands should quote the slipbox root."
  (let ((org-slipbox-directory "/tmp/org slipbox"))
    (should
     (equal
      (org-slipbox-buffer--unlinked-rg-command '("foo" "bar") "/tmp/regex")
      (concat
       "rg --follow --only-matching --vimgrep --pcre2 --ignore-case "
       "--glob \\*.org --glob \\*.org.gpg --glob \\*.org.age "
       "--file /tmp/regex /tmp/org\\ slipbox")))))

(ert-deftest org-slipbox-test-buffer-reflink-patterns-expand-citekeys ()
  "Reflink search should include cite: variants for citekeys."
  (should
   (equal
    (org-slipbox-buffer--reflink-patterns '("@smith2024" "https://example.com"))
    '("@smith2024" "cite:smith2024" "https://example.com"))))

(ert-deftest org-slipbox-test-buffer-dedicated-render-includes-discovery-sections ()
  "Dedicated buffers should render expensive discovery sections by default."
  (let ((org-slipbox-directory "/tmp")
        (org-slipbox-buffer-expensive-sections 'dedicated))
    (with-current-buffer (get-buffer-create "*org-slipbox: Note<note.org>*")
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        '(:node_key "file:note.org"
                          :title "Note"
                          :file_path "note.org"
                          :line 1
                          :kind "file"))
            (cl-letf (((symbol-function 'org-slipbox-buffer--backlinks)
                       (lambda (_node) nil))
                      ((symbol-function 'org-slipbox-buffer--reflinks)
                       (lambda (_node)
                         '((:file "/tmp/refs.org"
                            :row 3
                            :col 7
                            :preview "cite:smith2024"))))
                      ((symbol-function 'org-slipbox-buffer--unlinked-references)
                       (lambda (_node)
                         '((:file "/tmp/unlinked.org"
                            :row 9
                            :col 2
                            :preview "Note mention")))))
              (org-slipbox-buffer-render-contents))
            (should (string-match-p "Reflinks" (buffer-string)))
            (should (string-match-p "Unlinked References" (buffer-string)))
            (should (string-match-p "cite:smith2024" (buffer-string)))
            (should (string-match-p "Note mention" (buffer-string))))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-buffer-persistent-render-skips-expensive-sections ()
  "Persistent buffers should skip expensive discovery sections by default."
  (let ((org-slipbox-directory "/tmp")
        (org-slipbox-buffer-expensive-sections 'dedicated)
        (called nil))
    (with-current-buffer (get-buffer-create org-slipbox-buffer)
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        '(:node_key "file:note.org"
                          :title "Note"
                          :file_path "note.org"
                          :line 1
                          :kind "file"))
            (cl-letf (((symbol-function 'org-slipbox-buffer--backlinks)
                       (lambda (_node) nil))
                      ((symbol-function 'org-slipbox-buffer--reflinks)
                       (lambda (_node)
                         (setq called t)
                         nil))
                      ((symbol-function 'org-slipbox-buffer--unlinked-references)
                       (lambda (_node)
                         (setq called t)
                         nil)))
              (org-slipbox-buffer-render-contents))
            (should-not called))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-link-replace-at-point-rewrites-slipbox-links ()
  "Title-based org-slipbox links should rewrite to `id:' links."
  (with-temp-buffer
    (org-mode)
    (insert "[[slipbox:Heading]]")
    (goto-char (point-min))
    (search-forward "slipbox:Heading")
    (cl-letf (((symbol-function 'org-slipbox-node-from-title-or-alias)
               (lambda (_title-or-alias &optional _nocase)
                 '(:title "Heading" :file_path "note.org" :line 1)))
              ((symbol-function 'org-slipbox--ensure-node-id)
               (lambda (_node)
                 '(:title "Heading" :explicit_id "abc123"))))
      (org-slipbox-link-replace-at-point))
    (should (equal (buffer-string) "[[id:abc123][Heading]]"))))

(ert-deftest org-slipbox-test-link-replace-all-only-rewrites-slipbox-links ()
  "Bulk replacement should only touch org-slipbox links."
  (with-temp-buffer
    (org-mode)
    (insert "[[file:test.org][File]]\n[[slipbox:Heading]]\n[[https://example.com][Web]]")
    (let ((replace-count 0)
          (original-fn (symbol-function 'org-slipbox-link-replace-at-point)))
      (cl-letf (((symbol-function 'org-slipbox-link-replace-at-point)
                 (lambda ()
                   (cl-incf replace-count)
                   (funcall original-fn)))
                ((symbol-function 'org-slipbox-node-from-title-or-alias)
                 (lambda (_title-or-alias &optional _nocase)
                   '(:title "Heading" :file_path "note.org" :line 1)))
                ((symbol-function 'org-slipbox--ensure-node-id)
                 (lambda (_node)
                   '(:title "Heading" :explicit_id "abc123"))))
        (org-slipbox-link-replace-all)
        (should (= replace-count 1))
        (should (string-match-p "\\[\\[id:abc123\\]\\[Heading\\]\\]" (buffer-string)))))))

(ert-deftest org-slipbox-test-title-completion-candidates-use-indexed-search ()
  "Link completions should come from the indexed node search."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:nodes [(:title "Heading"
                           :aliases ["Head" "Alias"])
                          (:title "Other"
                           :aliases ["Hidden"])]))))
      (should (equal (org-slipbox--title-completion-candidates "He")
                     '("Heading" "Head"))))
    (should (equal method "slipbox/searchNodes"))
    (should (equal params '(:query "He" :limit 50)))))

(ert-deftest org-slipbox-test-refile-calls-rust-rpc-and-refreshes-buffers ()
  "Refile should delegate subtree movement to the Rust RPC layer."
  (let* ((root (make-temp-file "org-slipbox-refile-" t))
         (source (expand-file-name "source.org" root))
         (target (expand-file-name "target.org" root))
         sync-calls
         refresh-calls
         rpc-args)
    (unwind-protect
        (progn
          (write-region "" nil source nil 'silent)
          (write-region "" nil target nil 'silent)
          (with-current-buffer (find-file-noselect source)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "heading:source.org:3"
                             :file_path "source.org"
                             :line 3
                             :kind "heading"
                             :title "Move Me")))
                        ((symbol-function 'org-slipbox--sync-live-file-buffer-if-needed)
                         (lambda (path)
                           (push path sync-calls)))
                        ((symbol-function 'org-slipbox-rpc-refile-subtree)
                         (lambda (source-node-key target-node-key)
                           (setq rpc-args (list source-node-key target-node-key))
                           '(:node_key "heading:target.org:4")))
                        ((symbol-function 'org-slipbox--refresh-or-kill-file-buffer)
                         (lambda (path)
                           (push path refresh-calls))))
                (org-slipbox-refile
                 '(:node_key "heading:target.org:3"
                   :file_path "target.org"
                   :line 3
                   :kind "heading"
                   :title "Parent"))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            rpc-args
            '("heading:source.org:3" "heading:target.org:3")))
          (should
           (equal
            sync-calls
            (list target source)))
          (should
           (equal
            (nreverse refresh-calls)
            (list source target))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-refile-active-region-calls-rust-rpc-and-refreshes-buffers ()
  "Active-region refile should delegate movement to the Rust RPC layer."
  (let* ((root (make-temp-file "org-slipbox-refile-region-" t))
         (source (expand-file-name "source.org" root))
         (target (expand-file-name "target.org" root))
         sync-calls
         refresh-calls
         rpc-args)
    (unwind-protect
        (progn
          (write-region "Body\n" nil source nil 'silent)
          (write-region "" nil target nil 'silent)
          (with-current-buffer (find-file-noselect source)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'use-region-p) (lambda () t))
                        ((symbol-function 'region-beginning) (lambda () 2))
                        ((symbol-function 'region-end) (lambda () 5))
                        ((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "file:source.org"
                             :file_path "source.org"
                             :line 1
                             :kind "file"
                             :title "Source")))
                        ((symbol-function 'org-slipbox--sync-live-file-buffer-if-needed)
                         (lambda (path)
                           (push path sync-calls)))
                        ((symbol-function 'org-slipbox-rpc-refile-region)
                         (lambda (file-path start end target-node-key)
                           (setq rpc-args (list file-path start end target-node-key))
                           '(:node_key "heading:target.org:4")))
                        ((symbol-function 'org-slipbox--refresh-or-kill-file-buffer)
                         (lambda (path)
                           (push path refresh-calls))))
                (org-slipbox-refile
                 '(:node_key "heading:target.org:3"
                   :file_path "target.org"
                   :line 3
                   :kind "heading"
                   :title "Parent"))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            rpc-args
            (list source 2 5 "heading:target.org:3")))
          (should
           (equal
            sync-calls
            (list target source)))
          (should
           (equal
            (nreverse refresh-calls)
            (list source target))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-extract-subtree-calls-rust-rpc-and-refreshes-buffers ()
  "Extracting a subtree should delegate file mutation to the Rust RPC layer."
  (let* ((root (make-temp-file "org-slipbox-extract-" t))
         (source (expand-file-name "source.org" root))
         (target (expand-file-name "moved.org" root))
         rpc-args
         refresh-calls)
    (unwind-protect
        (progn
          (write-region "" nil source nil 'silent)
          (with-current-buffer (find-file-noselect source)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "heading:source.org:3"
                             :file_path "source.org"
                             :line 3
                             :kind "heading"
                             :title "Move Me")))
                        ((symbol-function 'org-slipbox-rpc-extract-subtree)
                         (lambda (source-node-key file-path)
                           (setq rpc-args (list source-node-key file-path))
                           '(:node_key "file:moved.org")))
                        ((symbol-function 'org-slipbox--refresh-or-kill-file-buffer)
                         (lambda (path)
                           (push path refresh-calls))))
                (should (equal (org-slipbox-extract-subtree target) target))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            rpc-args
            (list "heading:source.org:3" target)))
          (should
           (equal
           (nreverse refresh-calls)
           (list source target))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-demote-entire-buffer-calls-rust-rpc-and-refreshes-buffer ()
  "Whole-buffer demotion should delegate file mutation to the Rust RPC layer."
  (let* ((root (make-temp-file "org-slipbox-demote-" t))
         (file (expand-file-name "note.org" root))
         sync-calls
         refresh-calls
         rpc-path)
    (unwind-protect
        (progn
          (write-region "* Note\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox--sync-live-file-buffer-if-needed)
                         (lambda (path)
                           (push path sync-calls)))
                        ((symbol-function 'org-slipbox-rpc-demote-entire-file)
                         (lambda (path)
                           (setq rpc-path path)
                           '(:node_key "heading:note.org:1")))
                        ((symbol-function 'org-slipbox--refresh-or-kill-file-buffer)
                         (lambda (path)
                           (push path refresh-calls))))
                (should (equal (plist-get (org-slipbox-demote-entire-buffer) :node_key)
                               "heading:note.org:1"))))
            (kill-buffer (current-buffer)))
          (should (equal rpc-path file))
          (should (equal sync-calls (list file)))
          (should (equal refresh-calls (list file))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-promote-entire-buffer-calls-rust-rpc-and-refreshes-buffer ()
  "Whole-buffer promotion should delegate file mutation to the Rust RPC layer."
  (let* ((root (make-temp-file "org-slipbox-promote-" t))
         (file (expand-file-name "note.org" root))
         sync-calls
         refresh-calls
         rpc-path)
    (unwind-protect
        (progn
          (write-region "* Note\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox--sync-live-file-buffer-if-needed)
                         (lambda (path)
                           (push path sync-calls)))
                        ((symbol-function 'org-slipbox-rpc-promote-entire-file)
                         (lambda (path)
                           (setq rpc-path path)
                           '(:node_key "file:note.org")))
                        ((symbol-function 'org-slipbox--refresh-or-kill-file-buffer)
                         (lambda (path)
                           (push path refresh-calls))))
                (should (equal (plist-get (org-slipbox-promote-entire-buffer) :node_key)
                               "file:note.org"))))
            (kill-buffer (current-buffer)))
          (should (equal rpc-path file))
          (should (equal sync-calls (list file)))
          (should (equal refresh-calls (list file))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-ref-find-uses-rpc ()
  "Ref lookup should query the dedicated ref RPC."
  (let (method params visited)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:refs [(:reference "@smith2024"
                           :node (:title "Paper"
                                  :file_path "paper.org"
                                  :line 1))])))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node)
                 (setq visited node))))
      (org-slipbox-ref-find "smith"))
    (should (equal method "slipbox/searchRefs"))
    (should (equal params '(:query "smith" :limit 50)))
    (should (equal visited '(:title "Paper" :file_path "paper.org" :line 1)))))

(ert-deftest org-slipbox-test-tag-completions-merge-indexed-and-org-tags ()
  "Tag completions should include indexed and configured Org tags."
  (let ((org-tag-alist '((:startgroup . nil)
                         ("@work" . ?w)
                         (:endgroup . nil)
                         ("pc" . ?p)))
        method
        params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:tags ["alpha" "beta"]))))
      (should
       (equal
        (sort (org-slipbox-tag-completions) #'string-lessp)
        '("@work" "alpha" "beta" "pc"))))
    (should (equal method "slipbox/searchTags"))
    (should (equal params '(:query "" :limit 10000)))))

(ert-deftest org-slipbox-test-tag-add-uses-metadata-rpc ()
  "Adding a file tag should use the metadata RPC and refresh the file buffer."
  (let* ((root (make-temp-file "org-slipbox-tags-" t))
         (file (expand-file-name "note.org" root))
         params
         refreshed)
    (unwind-protect
        (progn
          (write-region "" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "file:note.org"
                             :file_path "note.org"
                             :line 1
                             :tags ["alpha"])))
                        ((symbol-function 'org-slipbox-rpc-update-node-metadata)
                         (lambda (request-params)
                           (setq params request-params)
                           '(:node_key "file:note.org")))
                        ((symbol-function 'org-slipbox--refresh-live-file-buffer)
                         (lambda (path)
                           (setq refreshed path))))
                (org-slipbox-tag-add '("beta"))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            params
            '(:node_key "file:note.org" :tags ("beta" "alpha"))))
          (should (equal refreshed file)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-tag-remove-uses-metadata-rpc ()
  "Removing a heading tag should use the metadata RPC and refresh the file buffer."
  (let* ((root (make-temp-file "org-slipbox-tags-" t))
         (file (expand-file-name "note.org" root))
         params
         refreshed)
    (unwind-protect
        (progn
          (write-region "" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root)
                  (org-auto-align-tags nil))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "heading:note.org:3"
                             :file_path "note.org"
                             :line 3
                             :tags ["one" "two"])))
                        ((symbol-function 'org-slipbox-rpc-update-node-metadata)
                         (lambda (request-params)
                           (setq params request-params)
                           '(:node_key "heading:note.org:3")))
                        ((symbol-function 'org-slipbox--refresh-live-file-buffer)
                         (lambda (path)
                           (setq refreshed path))))
                (org-slipbox-tag-remove '("one"))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            params
            '(:node_key "heading:note.org:3" :tags ("two"))))
          (should (equal refreshed file)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-agenda-day-range ()
  "Agenda day ranges should cover the full calendar day."
  (should
   (equal
    (org-slipbox-agenda--day-range (encode-time 0 0 0 7 3 2026))
    '("2026-03-07T00:00:00" . "2026-03-07T23:59:59"))))

(ert-deftest org-slipbox-test-agenda-date-uses-rpc ()
  "Agenda lookup should go through the indexed agenda RPC."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:nodes nil)))
              ((symbol-function 'display-buffer)
               (lambda (&rest _args) nil)))
      (org-slipbox-agenda-date (encode-time 0 0 0 7 3 2026)))
    (should (equal method "slipbox/agenda"))
    (should
     (equal params
            '(:start "2026-03-07T00:00:00"
              :end "2026-03-07T23:59:59")))))

(ert-deftest org-slipbox-test-ref-add-uses-metadata-rpc ()
  "Adding a ref should use the metadata RPC and refresh the file buffer."
  (let* ((root (make-temp-file "org-slipbox-ref-" t))
         (file (expand-file-name "note.org" root))
         params
         refreshed)
    (unwind-protect
        (progn
          (write-region "" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "file:note.org"
                             :file_path "note.org"
                             :line 1
                             :refs [])))
                        ((symbol-function 'org-slipbox-rpc-update-node-metadata)
                         (lambda (request-params)
                           (setq params request-params)
                           '(:node_key "file:note.org")))
                        ((symbol-function 'org-slipbox--refresh-live-file-buffer)
                         (lambda (path)
                           (setq refreshed path))))
                (org-slipbox-ref-add "http://site.net/docs/01. introduction - hello world.html")))
            (kill-buffer (current-buffer)))
          (should
           (equal
            params
            '(:node_key "file:note.org"
              :refs ("http://site.net/docs/01. introduction - hello world.html"))))
          (should (equal refreshed file)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-alias-add-uses-metadata-rpc ()
  "Adding an alias should use the metadata RPC and refresh the file buffer."
  (let* ((root (make-temp-file "org-slipbox-alias-" t))
         (file (expand-file-name "note.org" root))
         params
         refreshed)
    (unwind-protect
        (progn
          (write-region "" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:node_key "heading:note.org:3"
                             :file_path "note.org"
                             :line 3
                             :aliases [])))
                        ((symbol-function 'org-slipbox-rpc-update-node-metadata)
                         (lambda (request-params)
                           (setq params request-params)
                           '(:node_key "heading:note.org:3")))
                        ((symbol-function 'org-slipbox--refresh-live-file-buffer)
                         (lambda (path)
                           (setq refreshed path))))
                (org-slipbox-alias-add "Batman")))
            (kill-buffer (current-buffer)))
          (should
           (equal
            params
            '(:node_key "heading:note.org:3" :aliases ("Batman"))))
          (should (equal refreshed file)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-syncable-buffer-detection ()
  "Autosync should only consider eligible files under the configured root."
  (let* ((root (make-temp-file "org-slipbox-test-" t))
         (inside (expand-file-name "note.org" root))
         (inside-encrypted (expand-file-name "secret.org.gpg" root))
         (outside-root (make-temp-file "org-slipbox-outside-" t))
         (outside (expand-file-name "note.org" outside-root)))
    (unwind-protect
        (progn
          (write-region "" nil inside nil 'silent)
          (org-slipbox-test--write-literal-file inside-encrypted)
          (write-region "" nil outside nil 'silent)
          (let ((org-slipbox-directory root)
                (buffer-file-name inside))
            (should (org-slipbox--syncable-buffer-p))
            (let ((buffer-file-name inside-encrypted))
              (should (org-slipbox--syncable-buffer-p)))
            (let ((buffer-file-name outside))
              (should-not (org-slipbox--syncable-buffer-p)))))
      (delete-directory root t)
      (delete-directory outside-root t))))

(ert-deftest org-slipbox-test-autosync-mode-toggles-hooks-and-advices ()
  "Autosync mode should own its hooks, advices, and buffer-local save hook."
  (let* ((root (make-temp-file "org-slipbox-sync-" t))
         (file (expand-file-name "note.org" root))
         buffer)
    (unwind-protect
        (progn
          (write-region "" nil file nil 'silent)
          (setq buffer (find-file-noselect file))
          (let ((org-slipbox-directory root))
            (unwind-protect
                (progn
                  (org-slipbox-autosync-mode 1)
                  (should (memq #'org-slipbox--autosync-setup-file-h find-file-hook))
                  (should (advice-member-p #'org-slipbox--autosync-rename-file-a 'rename-file))
                  (should (advice-member-p #'org-slipbox--autosync-delete-file-a 'delete-file))
                  (should (advice-member-p #'org-slipbox--autosync-vc-delete-file-a 'vc-delete-file))
                  (with-current-buffer buffer
                    (should (memq #'org-slipbox-sync-current-buffer after-save-hook)))
                  (org-slipbox-autosync-mode -1)
                  (should-not (memq #'org-slipbox--autosync-setup-file-h find-file-hook))
                  (should-not (advice-member-p #'org-slipbox--autosync-rename-file-a 'rename-file))
                  (should-not (advice-member-p #'org-slipbox--autosync-delete-file-a 'delete-file))
                  (should-not (advice-member-p #'org-slipbox--autosync-vc-delete-file-a 'vc-delete-file))
                  (with-current-buffer buffer
                    (should-not (memq #'org-slipbox-sync-current-buffer after-save-hook))))
              (when org-slipbox-autosync-mode
                (org-slipbox-autosync-mode -1)))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-autosync-mode-syncs-on-save ()
  "Autosync mode should sync tracked buffers after save."
  (let* ((root (make-temp-file "org-slipbox-sync-" t))
         (file (expand-file-name "note.org" root))
         buffer
         calls)
    (unwind-protect
        (progn
          (write-region "Initial\n" nil file nil 'silent)
          (setq buffer (find-file-noselect file))
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-rpc-index-file)
                       (lambda (path)
                         (push (expand-file-name path) calls))))
              (unwind-protect
                  (progn
                    (org-slipbox-autosync-mode 1)
                    (with-current-buffer buffer
                      (goto-char (point-max))
                      (insert "Updated\n")
                      (save-buffer))
                    (should (equal calls (list file))))
                (when org-slipbox-autosync-mode
                  (org-slipbox-autosync-mode -1))))))
      (when (buffer-live-p buffer)
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-autosync-rename-updates-old-and-new-paths ()
  "Autosync mode should remove the old path and sync the renamed file."
  (let* ((root (make-temp-file "org-slipbox-sync-" t))
         (old (expand-file-name "old.org" root))
         (new (expand-file-name "new.org" root))
         calls)
    (unwind-protect
        (progn
          (write-region "Note\n" nil old nil 'silent)
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-rpc-index-file)
                       (lambda (path)
                         (push (expand-file-name path) calls))))
              (unwind-protect
                  (progn
                    (org-slipbox-autosync-mode 1)
                    (rename-file old new)
                    (should (equal (nreverse calls) (list old new))))
                (when org-slipbox-autosync-mode
                  (org-slipbox-autosync-mode -1))))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-autosync-delete-removes-indexed-file ()
  "Autosync mode should remove deleted files from the index."
  (let* ((root (make-temp-file "org-slipbox-sync-" t))
         (file (expand-file-name "delete.org" root))
         calls)
    (unwind-protect
        (progn
          (write-region "Note\n" nil file nil 'silent)
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-rpc-index-file)
                       (lambda (path)
                         (push (expand-file-name path) calls))))
              (unwind-protect
                  (progn
                    (org-slipbox-autosync-mode 1)
                    (delete-file file)
                    (should (equal calls (list file))))
                (when org-slipbox-autosync-mode
                  (org-slipbox-autosync-mode -1))))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-autosync-vc-delete-removes-indexed-file ()
  "VC delete handling should remove deleted files from the index."
  (let* ((root (make-temp-file "org-slipbox-sync-" t))
         (file (expand-file-name "vc-delete.org" root))
         calls)
    (unwind-protect
        (progn
          (write-region "Note\n" nil file nil 'silent)
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-rpc-index-file)
                       (lambda (path)
                         (push (expand-file-name path) calls))))
              (org-slipbox--autosync-vc-delete-file-a
               (lambda (target &rest _args)
                 (delete-file target))
               file)
              (should (equal calls (list file))))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-dailies-path-format ()
  "Daily note paths should stay relative to the slipbox root."
  (should
   (equal
    (let ((org-slipbox-dailies-directory "daily/"))
      (org-slipbox-dailies--path (encode-time 0 0 0 7 3 2026)))
    "daily/2026-03-07.org")))

(ert-deftest org-slipbox-test-dailies-goto-uses-rpc ()
  "Daily note lookup should go through the file-node RPC."
  (let (method params visited hook-ran)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "2026-03-07" :file_path "daily/2026-03-07.org" :line 1)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node)
                 (setq visited node))))
      (let ((org-slipbox-dailies-find-file-hook
             (list (lambda () (setq hook-ran t)))))
        (org-slipbox-dailies--goto (encode-time 0 0 0 7 3 2026))))
    (should (equal method "slipbox/ensureFileNode"))
    (should (equal params '(:file_path "daily/2026-03-07.org" :title "2026-03-07")))
    (should (equal visited '(:title "2026-03-07" :file_path "daily/2026-03-07.org" :line 1)))
    (should hook-ran)))

(ert-deftest org-slipbox-test-dailies-capture-uses-rpc ()
  "Daily entry capture should go through the append-heading RPC."
  (let (method params visited hook-ran)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 6)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node)
                 (setq visited node))))
      (let ((org-slipbox-dailies-entry-level 2)
            (org-slipbox-dailies-find-file-hook
             (list (lambda () (setq hook-ran t)))))
        (org-slipbox-dailies--capture (encode-time 0 0 0 7 3 2026) "Meeting")))
    (should (equal method "slipbox/appendHeading"))
    (should
     (equal params
            '(:file_path "daily/2026-03-07.org"
              :title "2026-03-07"
              :heading "Meeting"
              :level 2)))
    (should (equal visited '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 6)))
    (should hook-ran)))

(ert-deftest org-slipbox-test-dailies-capture-uses-template-targets ()
  "Daily entry capture should open a draft and visit on finalize."
  (let (buffer method params visited hook-ran)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 8)))
                    ((symbol-function 'org-slipbox--visit-node)
                     (lambda (node)
                       (setq visited node))))
            (let ((org-slipbox-dailies-capture-templates
                   '(("d" "default"
                      :target (file+head+olp "daily/%<%Y-%m-%d>.org"
                                             "#+title: %<%Y-%m-%d>\n"
                                             ("Inbox"))
                      :title "${title}")))
                  (org-slipbox-dailies-find-file-hook
                   (list (lambda () (setq hook-ran t)))))
              (setq buffer
                    (org-slipbox-dailies--capture
                     (encode-time 0 0 0 7 3 2026)
                     "Meeting"
                     "d"))
              (with-current-buffer buffer
                (goto-char (point-max))
                (insert "Agenda review")
                (org-slipbox-capture-finalize))))
          (should-not (buffer-live-p buffer))
          (should (equal method "slipbox/captureTemplate"))
          (should
           (equal params
                  '(:title "Meeting"
                    :capture_type "entry"
                    :content "Agenda review"
                    :prepend nil
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :file_path "daily/2026-03-07.org"
                    :head "#+title: 2026-03-07"
                    :outline_path ("Inbox"))))
          (should (equal visited '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 8)))
          (should hook-ran))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-dailies-list-files-filters-non-org-noise ()
  "Daily file listing should ignore dotfiles, autosaves, and backups."
  (let* ((root (make-temp-file "org-slipbox-dailies-" t))
         (daily (expand-file-name "daily" root)))
    (unwind-protect
        (progn
          (make-directory daily t)
          (write-region "" nil (expand-file-name "2026-03-07.org" daily) nil 'silent)
          (write-region "" nil (expand-file-name "2026-03-08.org" daily) nil 'silent)
          (org-slipbox-test--write-literal-file
           (expand-file-name "2026-03-09.org.gpg" daily))
          (write-region "" nil (expand-file-name ".hidden.org" daily) nil 'silent)
          (write-region "" nil (expand-file-name "#2026-03-09.org#" daily) nil 'silent)
          (write-region "" nil (expand-file-name "2026-03-10.org~" daily) nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-dailies-directory "daily/"))
            (should
             (equal
              (mapcar #'file-name-nondirectory (org-slipbox-dailies--list-files))
              '("2026-03-07.org" "2026-03-08.org" "2026-03-09.org.gpg")))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-dailies-daily-note-p-detects-daily-files ()
  "Daily file detection should be constrained to the dailies directory."
  (let* ((root (make-temp-file "org-slipbox-dailies-" t))
         (daily (expand-file-name "daily" root))
         (daily-file (expand-file-name "2026-03-07.org" daily))
         (other-file (expand-file-name "notes.org" root)))
    (unwind-protect
        (progn
          (make-directory daily t)
          (write-region "" nil daily-file nil 'silent)
          (write-region "" nil other-file nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-dailies-directory "daily/"))
            (should (org-slipbox-dailies--daily-note-p daily-file))
            (should-not (org-slipbox-dailies--daily-note-p other-file))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-dailies-goto-next-and-previous-note ()
  "Daily navigation should move across existing daily note files."
  (let* ((root (make-temp-file "org-slipbox-dailies-" t))
         (daily (expand-file-name "daily" root))
         (older (expand-file-name "2026-03-07.org" daily))
         (current (expand-file-name "2026-03-08.org" daily))
         (newer (expand-file-name "2026-03-09.org" daily))
         (visited nil)
         (hook-ran 0))
    (unwind-protect
        (progn
          (make-directory daily t)
          (write-region "#+title: 2026-03-07\n" nil older nil 'silent)
          (write-region "#+title: 2026-03-08\n" nil current nil 'silent)
          (write-region "#+title: 2026-03-09\n" nil newer nil 'silent)
          (with-current-buffer (find-file-noselect current)
            (let ((org-slipbox-directory root)
                  (org-slipbox-dailies-directory "daily/")
                  (org-slipbox-dailies-find-file-hook
                   (list (lambda () (setq hook-ran (1+ hook-ran))))))
              (org-slipbox-dailies-goto-next-note)
              (setq visited (buffer-file-name))
              (org-slipbox-dailies-goto-previous-note)
              (setq visited (cons visited (buffer-file-name))))
            (mapc (lambda (buffer)
                    (when (buffer-live-p buffer)
                      (kill-buffer buffer)))
                  (list (current-buffer)
                        (find-buffer-visiting older)
                        (find-buffer-visiting newer))))
          (should (equal visited (cons newer current)))
          (should (= hook-ran 2)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-dailies-calendar-file-to-date ()
  "Daily file names should map onto calendar date triples."
  (should
   (equal
    (org-slipbox-dailies-calendar--file-to-date "/tmp/daily/2026-03-07.org")
    '(3 7 2026)))
  (should
   (equal
    (org-slipbox-dailies-calendar--file-to-date "/tmp/daily/2026-03-07.org.gpg")
    '(3 7 2026)))
  (should-not
   (org-slipbox-dailies-calendar--file-to-date "/tmp/daily/not-a-date.org")))

(ert-deftest org-slipbox-test-dailies-calendar-mark-entries-marks-visible-dates ()
  "Calendar marking should only mark parseable visible daily notes."
  (require 'calendar)
  (let* ((root (make-temp-file "org-slipbox-dailies-" t))
         (daily-dir (expand-file-name "daily" root))
         marks)
    (unwind-protect
        (progn
          (make-directory daily-dir t)
          (write-region "" nil (expand-file-name "2026-03-07.org" daily-dir) nil 'silent)
          (write-region "" nil (expand-file-name "2026-03-08.org" daily-dir) nil 'silent)
          (write-region "" nil (expand-file-name "scratch.org" daily-dir) nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-dailies-directory "daily/"))
            (cl-letf (((symbol-function 'calendar-date-is-visible-p)
                       (lambda (date)
                         (equal date '(3 7 2026))))
                      ((symbol-function 'calendar-mark-visible-date)
                       (lambda (date face)
                         (push (list date face) marks))))
              (org-slipbox-dailies-calendar-mark-entries)))
          (should
           (equal marks
                  '(((3 7 2026) org-slipbox-dailies-calendar-note)))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-dailies-calendar-mode-toggles-hooks ()
  "Calendar integration should be opt-in rather than installed at load time."
  (require 'calendar)
  (let ((calendar-today-visible-hook nil)
        (calendar-today-invisible-hook nil)
        (org-slipbox-dailies-calendar-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-dailies-calendar-mode 1)
          (should (memq #'org-slipbox-dailies-calendar-mark-entries
                        calendar-today-visible-hook))
          (should (memq #'org-slipbox-dailies-calendar-mark-entries
                        calendar-today-invisible-hook))
          (org-slipbox-dailies-calendar-mode -1)
          (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                            calendar-today-visible-hook))
          (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                            calendar-today-invisible-hook)))
      (setq org-slipbox-dailies-calendar-mode nil))))

(ert-deftest org-slipbox-test-protocol-mode-toggles-handlers ()
  "Protocol mode should register and remove handlers explicitly."
  (require 'org-protocol)
  (let ((org-protocol-protocol-alist nil))
    (unwind-protect
        (progn
          (org-slipbox-protocol-mode 1)
          (should (assoc "org-slipbox-ref" org-protocol-protocol-alist))
          (should (assoc "org-slipbox-node" org-protocol-protocol-alist))
          (org-slipbox-protocol-mode 0)
          (should-not (assoc "org-slipbox-ref" org-protocol-protocol-alist))
          (should-not (assoc "org-slipbox-node" org-protocol-protocol-alist)))
      (org-slipbox-protocol-mode 0))))

(ert-deftest org-slipbox-test-protocol-open-node-visits-indexed-node ()
  "Node protocol should resolve and visit the indexed node."
  (let (id visited raised)
    (cl-letf (((symbol-function 'org-slipbox-node-from-id)
               (lambda (node-id)
                 (setq id node-id)
                 '(:title "Node" :file_path "node.org" :line 3)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node &optional _other-window)
                 (setq visited node)))
              ((symbol-function 'raise-frame)
               (lambda ()
                 (setq raised t))))
      (org-slipbox-protocol-open-node '(:node "abc%20123")))
    (should (equal id "abc 123"))
    (should (equal visited '(:title "Node" :file_path "node.org" :line 3)))
    (should raised)))

(ert-deftest org-slipbox-test-protocol-open-ref-uses-ref-capture-templates ()
  "Ref protocol should reuse ref capture templates and protocol context."
  (let (captured stored-props raised)
    (require 'org-protocol)
    (let ((org-slipbox-protocol-store-links t)
          (org-stored-links nil))
      (cl-letf (((symbol-function 'org-slipbox-capture-ref)
                 (lambda (reference title templates keys variables)
                   (setq captured (list :reference reference
                                        :title title
                                        :templates templates
                                        :keys keys
                                        :variables variables))
                   '(:title "Article" :file_path "article.org" :line 1)))
                ((symbol-function 'org-link-store-props)
                 (lambda (&rest props)
                   (setq stored-props props)))
                ((symbol-function 'raise-frame)
                 (lambda ()
                   (setq raised t))))
        (org-slipbox-protocol-open-ref
         '(:template "r"
           :ref "https%3A%2F%2Fexample.test%2Farticle"
           :title "An%20Article"
           :body "Selected%20text")))
      (should raised)
      (should (equal org-stored-links '(("https://example.test/article" "An Article"))))
      (should
       (equal captured
              (list :reference "https://example.test/article"
                    :title "An Article"
                    :templates org-slipbox-capture-ref-templates
                    :keys "r"
                    :variables
                    '(:ref "https://example.test/article"
                      :body "Selected text"
                      :annotation "[[https://example.test/article][An Article]]"
                      :link "https://example.test/article"))))
      (should (equal (plist-get stored-props :link) "https://example.test/article"))
      (should (equal (plist-get stored-props :initial) "Selected text")))))

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
