;;; test-org-slipbox.el --- Tests for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;;; Commentary:

;; Basic smoke tests for the Elisp package.

;;; Code:

(require 'cl-lib)
(require 'ert)
(require 'org-slipbox)

(ert-deftest org-slipbox-test-feature-provided ()
  "The package entry feature should load cleanly."
  (should (featurep 'org-slipbox)))

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

(ert-deftest org-slipbox-test-capture-template-expansion ()
  "Capture templates should expand slug, title, and time placeholders."
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
    "Slip: Sample Title")))

(ert-deftest org-slipbox-test-capture-node-uses-template-path ()
  "Capture should send expanded file targets through the RPC layer."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:title "Slip: Sample Title" :file_path "notes/2026-sample-title.org" :line 1))))
      (org-slipbox--capture-node
       "Sample Title"
       '("d" "default" :path "notes/%<%Y>-${slug}.org" :title "Slip: ${title}")))
    (should (equal method "slipbox/captureNode"))
    (should
     (equal params
            '(:title "Slip: Sample Title"
              :file_path "notes/2026-sample-title.org")))))

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
                       '(:backlinks [(:title "Backlink"
                                     :file_path "other.org"
                                     :line 10)]))))
            (org-slipbox-buffer-render-contents))
          (should (derived-mode-p 'org-slipbox-buffer-mode))
          (should (string-match-p "Refs" (buffer-string)))
          (should (string-match-p "@smith2024" (buffer-string)))
          (should (string-match-p "Backlinks" (buffer-string)))
          (should (string-match-p "Backlink" (buffer-string))))
      (kill-buffer (current-buffer)))))

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

(ert-deftest org-slipbox-test-tag-add-updates-filetags-keyword ()
  "Adding a file tag should update `#+FILETAGS:' and sync the file."
  (let* ((root (make-temp-file "org-slipbox-tags-" t))
         (file (expand-file-name "note.org" root))
         method
         params)
    (unwind-protect
        (progn
          (write-region "#+title: Note\n\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-min))
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (request-method request-params)
                           (setq method request-method
                                 params request-params)
                           nil)))
                (org-slipbox-tag-add '("beta"))))
            (kill-buffer (current-buffer)))
          (should (equal method "slipbox/indexFile"))
          (should (equal params `(:file_path ,file)))
          (should
           (equal
            (with-temp-buffer
              (insert-file-contents file)
              (buffer-string))
            "#+title: Note\n#+FILETAGS: :beta:\n\n")))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-tag-remove-updates-heading-tags ()
  "Removing a heading tag should rewrite the headline and sync the file."
  (let* ((root (make-temp-file "org-slipbox-tags-" t))
         (file (expand-file-name "note.org" root)))
    (unwind-protect
        (progn
          (write-region "#+title: Note\n\n* Heading :one:two:\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-min))
            (search-forward "* Heading")
            (let ((org-slipbox-directory root)
                  (org-auto-align-tags nil))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (&rest _args) nil)))
                (org-slipbox-tag-remove '("one"))))
            (kill-buffer (current-buffer)))
          (should
           (equal
            (with-temp-buffer
              (insert-file-contents file)
              (buffer-string))
            "#+title: Note\n\n* Heading :two:\n")))
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

(ert-deftest org-slipbox-test-ref-add-updates-file-property ()
  "Adding a ref should update the file-level property drawer and sync the file."
  (let* ((root (make-temp-file "org-slipbox-ref-" t))
         (file (expand-file-name "note.org" root))
         method
         params)
    (unwind-protect
        (progn
          (write-region "#+title: Note\n\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-min))
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (request-method request-params)
                           (setq method request-method
                                 params request-params)
                           nil)))
                (org-slipbox-ref-add "http://site.net/docs/01. introduction - hello world.html")))
            (kill-buffer (current-buffer)))
          (should (equal method "slipbox/indexFile"))
          (should (equal params `(:file_path ,file)))
          (should
           (equal
            (with-temp-buffer
              (insert-file-contents file)
              (buffer-string))
            "#+title: Note\n:PROPERTIES:\n:ROAM_REFS: \"http://site.net/docs/01. introduction - hello world.html\"\n:END:\n\n")))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-alias-add-updates-heading-property ()
  "Adding an alias should update the current heading property drawer."
  (let* ((root (make-temp-file "org-slipbox-alias-" t))
         (file (expand-file-name "note.org" root)))
    (unwind-protect
        (progn
          (write-region "#+title: Note\n\n* Heading\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-min))
            (search-forward "* Heading")
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (&rest _args) nil)))
                (org-slipbox-alias-add "Batman")))
            (kill-buffer (current-buffer)))
          (should
           (equal
            (with-temp-buffer
              (insert-file-contents file)
              (buffer-string))
            "#+title: Note\n\n* Heading\n:PROPERTIES:\n:ROAM_ALIASES: Batman\n:END:\n")))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-syncable-buffer-detection ()
  "Autosync should only consider Org files under the configured root."
  (let* ((root (make-temp-file "org-slipbox-test-" t))
         (inside (expand-file-name "note.org" root))
         (outside-root (make-temp-file "org-slipbox-outside-" t))
         (outside (expand-file-name "note.org" outside-root)))
    (unwind-protect
        (progn
          (write-region "" nil inside nil 'silent)
          (write-region "" nil outside nil 'silent)
          (let ((org-slipbox-directory root)
                (buffer-file-name inside))
            (should (org-slipbox--syncable-buffer-p))
            (let ((buffer-file-name outside))
              (should-not (org-slipbox--syncable-buffer-p)))))
      (delete-directory root t)
      (delete-directory outside-root t))))

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

(ert-deftest org-slipbox-test-dailies-list-files-filters-non-org-noise ()
  "Daily file listing should ignore dotfiles, autosaves, and backups."
  (let* ((root (make-temp-file "org-slipbox-dailies-" t))
         (daily (expand-file-name "daily" root)))
    (unwind-protect
        (progn
          (make-directory daily t)
          (write-region "" nil (expand-file-name "2026-03-07.org" daily) nil 'silent)
          (write-region "" nil (expand-file-name "2026-03-08.org" daily) nil 'silent)
          (write-region "" nil (expand-file-name ".hidden.org" daily) nil 'silent)
          (write-region "" nil (expand-file-name "#2026-03-09.org#" daily) nil 'silent)
          (write-region "" nil (expand-file-name "2026-03-10.org~" daily) nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-dailies-directory "daily/"))
            (should
             (equal
              (mapcar #'file-name-nondirectory (org-slipbox-dailies--list-files))
              '("2026-03-07.org" "2026-03-08.org")))))
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

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
