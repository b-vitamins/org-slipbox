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

(ert-deftest org-slipbox-test-a-package-exposes-optional-entrypoints-via-autoloads ()
  "Optional command entry points should be available after top-level load."
  (should (fboundp 'org-slipbox-export-mode))
  (should (autoloadp (symbol-function 'org-slipbox-export-mode)))
  (should-not (featurep 'org-slipbox-export))
  (should (fboundp 'org-slipbox-graph))
  (should (autoloadp (symbol-function 'org-slipbox-graph)))
  (should-not (featurep 'org-slipbox-graph)))

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
  (should-not (memq #'org-slipbox-mode--maybe-enable-completion find-file-hook))
  (should-not (advice-member-p #'org-slipbox--autosync-rename-file-a 'rename-file))
  (should-not (advice-member-p #'org-slipbox--autosync-delete-file-a 'delete-file))
  (should-not (advice-member-p #'org-slipbox--autosync-vc-delete-file-a 'vc-delete-file))
  (should-not (advice-member-p #'org-slipbox-id-find 'org-id-find))
  (should-not (memq #'org-slipbox-buffer--redisplay-h post-command-hook))
  (should-not org-slipbox-mode)
  (should-not org-slipbox-buffer-persistent-mode)
  (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                    calendar-today-visible-hook))
  (should-not (memq #'org-slipbox-dailies-calendar-mark-entries
                    calendar-today-invisible-hook))
  (should-not (assoc "org-slipbox-ref" org-protocol-protocol-alist))
  (should-not (assoc "org-slipbox-node" org-protocol-protocol-alist)))

(ert-deftest org-slipbox-test-rpc-request-normalizes-list-values ()
  "RPC transport should turn plain Elisp lists into JSON arrays."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-ensure)
               (lambda () :connection))
              ((symbol-function 'jsonrpc-request)
               (lambda (_connection request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:ok t))))
      (org-slipbox-rpc-update-node-metadata
       '(:node_key "file:note.org"
         :aliases ("Batman")
         :tags ("hero" "gotham"))))
    (should (equal method "slipbox/updateNodeMetadata"))
    (should
     (equal params
            '(:node_key "file:note.org"
              :aliases ["Batman"]
              :tags ["hero" "gotham"])))))

(ert-deftest org-slipbox-test-rpc-request-normalizes-capture-ref-lists ()
  "Capture RPC transport should normalize ref lists for JSON encoding."
  (let (params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-ensure)
               (lambda () :connection))
              ((symbol-function 'jsonrpc-request)
               (lambda (_connection _request-method request-params)
                 (setq params request-params)
                 '(:ok t))))
      (org-slipbox-rpc-capture-template
       '(:title "Web Clip"
         :capture_type "plain"
         :content "Body"
         :refs ("https://example.invalid/web-clip")
         :prepend :json-false)))
    (should
     (equal params
            '(:title "Web Clip"
              :capture_type "plain"
              :content "Body"
              :refs ["https://example.invalid/web-clip"]
              :prepend :json-false)))))

(ert-deftest org-slipbox-test-global-mode-owns-setup-hooks-and-modes ()
  "The recommended setup mode should own its hooks and managed modes."
  (let ((org-slipbox-mode nil)
        (org-slipbox-autosync-mode nil)
        (org-slipbox-id-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-mode 1)
          (should (memq #'org-slipbox-mode--maybe-enable-completion find-file-hook))
          (should org-slipbox-autosync-mode)
          (should org-slipbox-id-mode)
          (org-slipbox-mode -1)
          (should-not (memq #'org-slipbox-mode--maybe-enable-completion find-file-hook))
          (should-not org-slipbox-autosync-mode)
          (should-not org-slipbox-id-mode))
      (when org-slipbox-mode
        (org-slipbox-mode -1))
      (when org-slipbox-autosync-mode
        (org-slipbox-autosync-mode -1))
      (when org-slipbox-id-mode
        (org-slipbox-id-mode -1)))))

(ert-deftest org-slipbox-test-global-mode-does-not-disable-user-enabled-modes ()
  "The recommended setup mode should not turn off modes it did not enable."
  (let ((org-slipbox-mode nil)
        (org-slipbox-autosync-mode nil)
        (org-slipbox-id-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-autosync-mode 1)
          (org-slipbox-id-mode 1)
          (org-slipbox-mode 1)
          (org-slipbox-mode -1)
          (should org-slipbox-autosync-mode)
          (should org-slipbox-id-mode))
      (when org-slipbox-mode
        (org-slipbox-mode -1))
      (when org-slipbox-autosync-mode
        (org-slipbox-autosync-mode -1))
      (when org-slipbox-id-mode
        (org-slipbox-id-mode -1)))))

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

(ert-deftest org-slipbox-test-org-id-find-falls-back-to-org-id-locations-for-node-excluded-files ()
  "The org-id bridge should preserve fallback for node-excluded file IDs."
  (require 'org-id)
  (let* ((root (make-temp-file "org-slipbox-id-" t))
         (excluded (expand-file-name "excluded.org" root))
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
          (write-region ":PROPERTIES:\n:ID: excluded-id\n:ROAM_EXCLUDE: t\n:END:\n#+title: Excluded\n"
                        nil excluded nil 'silent)
          (let ((org-slipbox-directory root))
            (org-slipbox-update-org-id-locations)
            (should (equal (gethash "excluded-id" org-id-locations)
                           (abbreviate-file-name excluded)))
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
      (delete-directory root t))))

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

(ert-deftest org-slipbox-test-search-files-uses-indexed-rpc-results ()
  "Indexed file search should use the dedicated RPC surface."
  (let (rpc-args)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-files)
               (lambda (query limit)
                 (setq rpc-args (list query limit))
                 '(:files [(:file_path "notes/beta.org"
                           :title "Project Beta"
                           :mtime_ns 42
                           :node_count 2)]))))
      (should
      (equal
       (org-slipbox-search-files "beta" 25)
       '((:file_path "notes/beta.org"
           :title "Project Beta"
           :mtime_ns 42
           :node_count 2)))))
    (should (equal rpc-args '("beta" 25)))))

(ert-deftest org-slipbox-test-search-occurrences-uses-indexed-rpc-results ()
  "Occurrence search should use the dedicated RPC surface."
  (let (rpc-args)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-occurrences)
               (lambda (query limit)
                 (setq rpc-args (list query limit))
                 '(:occurrences [(:file_path "notes/beta.org"
                                  :row 7
                                  :col 4
                                  :preview "Needle in body."
                                  :matched_text "Needle"
                                  :owning_node (:title "Project Beta"))]))))
      (should
       (equal
        (org-slipbox-search-occurrences "needle" 25)
        '((:file_path "notes/beta.org"
           :row 7
           :col 4
           :preview "Needle in body."
           :matched_text "Needle"
           :owning_node (:title "Project Beta"))))))
    (should (equal rpc-args '("needle" 25)))))

(ert-deftest org-slipbox-test-discovery-policy-normalizes-command-args ()
  "Discovery helpers should normalize extensions and exclusions once."
  (let ((org-slipbox-file-extensions '("org" ".md" "ORG" ""))
        (org-slipbox-file-exclude-regexp '("^archive/" "" "  " "\\.cache/")))
    (should (equal (org-slipbox-discovery-file-extensions)
                   '("org" "md")))
    (should (equal (org-slipbox-discovery-exclude-regexps)
                   '("^archive/" "\\.cache/")))
    (should (equal (org-slipbox-discovery-command-args)
                   '("--file-extension" "org"
                     "--file-extension" "md"
                     "--exclude-regexp" "^archive/"
                     "--exclude-regexp" "\\.cache/")))))

(ert-deftest org-slipbox-test-rpc-command-includes-discovery-policy ()
  "Daemon startup should include the configured discovery policy."
  (let ((org-slipbox-server-program "/tmp/slipbox")
        (org-slipbox-directory "/tmp/notes")
        (org-slipbox-database-file "/tmp/org-slipbox.sqlite")
        (org-slipbox-file-extensions '("org" ".md"))
        (org-slipbox-file-exclude-regexp '("^archive/" "\\.cache/")))
    (cl-letf (((symbol-function 'file-exists-p)
               (lambda (path)
                 (string= path "/tmp/slipbox")))
              ((symbol-function 'file-executable-p)
               (lambda (path)
                 (string= path "/tmp/slipbox"))))
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
          "--exclude-regexp" "\\.cache/"))))))

(ert-deftest org-slipbox-test-rpc-resolves-daemon-from-path ()
  "Daemon startup should resolve PATH-based executables once."
  (let ((org-slipbox-server-program "slipbox")
        (org-slipbox-directory "/tmp/notes")
        (org-slipbox-database-file "/tmp/org-slipbox.sqlite"))
    (cl-letf (((symbol-function 'executable-find)
               (lambda (program)
                 (and (string= program "slipbox") "/usr/bin/slipbox")))
              ((symbol-function 'file-exists-p)
               (lambda (path)
                 (string= path "/usr/bin/slipbox")))
              ((symbol-function 'file-executable-p)
               (lambda (path)
                 (string= path "/usr/bin/slipbox"))))
      (should
       (equal
        (org-slipbox-rpc--command)
        '("/usr/bin/slipbox"
          "serve"
          "--root" "/tmp/notes"
          "--db" "/tmp/org-slipbox.sqlite"
          "--file-extension" "org"))))))

(ert-deftest org-slipbox-test-rpc-ensure-errors-when-daemon-is-missing ()
  "Daemon startup should fail clearly when the binary cannot be found."
  (let ((org-slipbox-server-program "slipbox")
        (org-slipbox-directory temporary-file-directory)
        (org-slipbox-database-file "/tmp/org-slipbox.sqlite"))
    (cl-letf (((symbol-function 'executable-find) (lambda (_program) nil)))
      (let ((error-data (should-error
                         (org-slipbox-rpc-ensure)
                         :type 'user-error
                         :exclude-subtypes nil)))
        (should
         (string-match-p
          "Cannot find the slipbox daemon"
          (cadr error-data)))
        (should
         (string-match-p
          "PATH"
          (cadr error-data)))
        (should
         (string-match-p
          "org-slipbox-server-program"
          (cadr error-data)))))))

(ert-deftest org-slipbox-test-rpc-ensure-errors-when-daemon-is-not-executable ()
  "Daemon startup should fail clearly for a non-executable daemon path."
  (let ((org-slipbox-server-program "/tmp/slipbox")
        (org-slipbox-directory temporary-file-directory)
        (org-slipbox-database-file "/tmp/org-slipbox.sqlite"))
    (cl-letf (((symbol-function 'file-exists-p)
               (lambda (path)
                 (string= path "/tmp/slipbox")))
              ((symbol-function 'file-executable-p)
               (lambda (_path) nil)))
      (let ((error-data (should-error
                         (org-slipbox-rpc-ensure)
                         :type 'user-error
                         :exclude-subtypes nil)))
        (should
         (equal
          (cadr error-data)
          "The slipbox daemon at /tmp/slipbox is not executable"))))))

(ert-deftest org-slipbox-test-rebuild-resets-daemon-and-deletes-database-files ()
  "Rebuild should reset the daemon connection and delete SQLite files first."
  (let* ((root (make-temp-file "org-slipbox-maint-" t))
         (database (expand-file-name "org-slipbox.sqlite" root))
         reset-called
         index-called)
    (unwind-protect
        (progn
          (write-region "" nil database nil 'silent)
          (write-region "" nil (concat database "-wal") nil 'silent)
          (write-region "" nil (concat database "-shm") nil 'silent)
          (let ((org-slipbox-database-file database))
            (cl-letf (((symbol-function 'org-slipbox-rpc-reset)
                       (lambda ()
                         (setq reset-called t)))
                      ((symbol-function 'org-slipbox-rpc-index)
                       (lambda ()
                         (setq index-called t)
                         '(:files_indexed 1 :nodes_indexed 2 :links_indexed 3))))
              (org-slipbox-rebuild)))
          (should reset-called)
          (should index-called)
          (should-not (file-exists-p database))
          (should-not (file-exists-p (concat database "-wal")))
          (should-not (file-exists-p (concat database "-shm"))))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-sync-current-file-saves-and-indexes-buffer ()
  "Explicit current-file sync should save the buffer before indexing it."
  (let* ((root (make-temp-file "org-slipbox-maint-" t))
         (file (expand-file-name "note.org" root))
         method
         params)
    (unwind-protect
        (progn
          (write-region "* Note\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (goto-char (point-max))
            (insert "Body\n")
            (let ((org-slipbox-directory root))
              (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                         (lambda (request-method request-params)
                           (setq method request-method
                                 params request-params)
                           '(:file_path "note.org"))))
                (org-slipbox-sync-current-file)
                (should-not (buffer-modified-p))))
            (kill-buffer (current-buffer)))
          (should (equal method "slipbox/indexFile"))
          (should (equal params `(:file_path ,file)))
          (with-temp-buffer
            (insert-file-contents file)
            (should (string-match-p "Body" (buffer-string)))))
      (delete-directory root t))))

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

(ert-deftest org-slipbox-test-node-candidate-display-supports-metadata-template-fields ()
  "Node candidate templates should interpolate indexed metadata fields."
  (let* ((mtime-ns 1741353600000000000)
         (expected-mtime
          (format-time-string
           "%Y-%m-%d"
           (seconds-to-time (/ (float mtime-ns) 1000000000.0)))))
    (should
      (equal
      (org-slipbox--format-node-template
       "${title} ${backlinkscount} ${forwardlinkscount} ${modtime} ${mtime-ns}"
       `(:title "Heading"
         :backlink_count 3
         :forward_link_count 5
         :file_mtime_ns ,mtime-ns)
       80)
      (format "Heading 3 5 %s %s" expected-mtime mtime-ns)))))

(ert-deftest org-slipbox-test-search-node-choices-use-configured-display-template ()
  "Interactive node choices should use the configured candidate formatter."
  (let ((org-slipbox-node-display-template
         (lambda (node)
           (format "Choice: %s" (plist-get node :title)))))
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional _sort)
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

(ert-deftest org-slipbox-test-node-read-delegates-default-sort-and-annotation ()
  "Node read should delegate supported sorts and expose annotation metadata."
  (let ((org-slipbox-node-default-sort 'title)
        (org-slipbox-node-annotation-function
         (lambda (node)
           (format " [%s|%s|%s]"
                   (plist-get node :file_path)
                   (plist-get node :backlink_count)
                   (plist-get node :forward_link_count))))
        rpc-sort
        metadata
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional sort)
                 (setq rpc-sort sort)
                 (if (eq sort 'title)
                     '(:nodes [(:title "Alpha" :file_path "a.org" :line 1
                                :backlink_count 7 :forward_link_count 3
                                :file_mtime_ns 1741353600000000000)
                               (:title "Zulu" :file_path "z.org" :line 2
                                :backlink_count 1 :forward_link_count 0
                                :file_mtime_ns 1741267200000000000)])
                   '(:nodes [(:title "Zulu" :file_path "z.org" :line 2
                              :backlink_count 1 :forward_link_count 0
                              :file_mtime_ns 1741267200000000000)
                             (:title "Alpha" :file_path "a.org" :line 1
                              :backlink_count 7 :forward_link_count 3
                              :file_mtime_ns 1741353600000000000)]))))
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
        (should (eq rpc-sort 'title))
        (should (eq (cdr (assq 'display-sort-function props)) 'identity))
        (should (equal annotation " [a.org|7|3]"))))))

(ert-deftest org-slipbox-test-node-read-file-mtime-sort-avoids-file-attributes ()
  "Supported file-mtime sorting should be delegated to the daemon."
  (let ((org-slipbox-node-default-sort 'file-mtime)
        rpc-sort
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional sort)
                 (setq rpc-sort sort)
                 '(:nodes [(:title "Newest" :file_path "new.org" :line 1)
                           (:title "Older" :file_path "old.org" :line 1)])))
              ((symbol-function 'file-attributes)
               (lambda (&rest _args)
                 (ert-fail "supported sorts should not call file-attributes")))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil))
                 (car candidates))))
      (let ((node (org-slipbox-node-read)))
        (should (eq rpc-sort 'file-mtime))
        (should (equal (plist-get node :title) "Newest"))))))

(ert-deftest org-slipbox-test-node-read-metadata-display-template-avoids-file-attributes ()
  "Metadata-backed display templates should not stat files on the hot path."
  (let* ((mtime-ns 1741353600000000000)
         (expected-mtime
          (format-time-string
           "%Y-%m-%d"
           (seconds-to-time (/ (float mtime-ns) 1000000000.0))))
         (org-slipbox-node-default-sort 'title)
         (org-slipbox-node-display-template
          "${title} ${modtime} ${backlinks} ${forward-links}")
         rpc-sort
         candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional sort)
                 (setq rpc-sort sort)
                 `(:nodes [(:title "Newest"
                            :file_path "new.org"
                            :line 1
                            :file_mtime_ns ,mtime-ns
                            :backlink_count 4
                            :forward_link_count 2)])))
              ((symbol-function 'file-attributes)
               (lambda (&rest _args)
                 (ert-fail "metadata-backed display should not call file-attributes")))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil))
                 (car candidates))))
      (let ((node (org-slipbox-node-read)))
        (should (eq rpc-sort 'title))
        (should (equal (substring-no-properties (car candidates))
                       (format "Newest %s 4 2" expected-mtime)))
        (should (equal (plist-get node :title) "Newest"))))))

(ert-deftest org-slipbox-test-node-read-file-atime-sort-errors ()
  "Unsupported legacy `file-atime' sorting should error clearly."
  (let ((org-slipbox-node-default-sort 'file-atime))
    (should-error (org-slipbox-node-read) :type 'user-error)))

(ert-deftest org-slipbox-test-node-read-custom-sort-remains-local ()
  "Custom sort comparators should still run client-side."
  (let (rpc-sort
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional sort)
                 (setq rpc-sort sort)
                 '(:nodes [(:title "Zulu" :file_path "z.org" :line 2)
                           (:title "Alpha" :file_path "a.org" :line 1)])))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil))
                 (car candidates))))
      (let ((node (org-slipbox-node-read nil nil #'org-slipbox-node-read-sort-by-title)))
        (should (null rpc-sort))
        (should (equal (plist-get node :title) "Alpha"))))))

(ert-deftest org-slipbox-test-node-read-custom-file-mtime-sort-uses-indexed-metadata ()
  "Built-in file-mtime comparator should use indexed metadata, not file stats."
  (let (rpc-sort
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-nodes)
               (lambda (_query _limit &optional sort)
                 (setq rpc-sort sort)
                 '(:nodes [(:title "Older"
                            :file_path "old.org"
                            :line 2
                            :file_mtime_ns 10)
                           (:title "Newest"
                            :file_path "new.org"
                            :line 1
                            :file_mtime_ns 20)])))
              ((symbol-function 'file-attributes)
               (lambda (&rest _args)
                 (ert-fail "custom file-mtime sort should not call file-attributes")))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil))
                 (car candidates))))
      (let ((node (org-slipbox-node-read nil nil #'org-slipbox-node-read-sort-by-file-mtime)))
        (should (null rpc-sort))
        (should (equal (substring-no-properties (car candidates))
                       "Newest | new.org:1"))
        (should (equal (plist-get node :title) "Newest"))))))

(ert-deftest org-slipbox-test-ref-read-exposes-dedicated-chooser-metadata ()
  "Ref read should expose annotation metadata, history, and duplicate refs."
  (let ((org-slipbox-ref-annotation-function
         (lambda (entry)
           (format " [%s]" (plist-get (plist-get entry :node) :title))))
        method
        params
        prompt
        history
        metadata
        candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:refs [(:reference "@smith2024"
                           :node (:title "Paper A"
                                  :file_path "a.org"
                                  :line 1
                                  :node_key "file:a.org"))
                          (:reference "@smith2024"
                           :node (:title "Paper B"
                                  :file_path "b.org"
                                  :line 2
                                  :node_key "file:b.org"))])))
              ((symbol-function 'completing-read)
               (lambda (read-prompt collection _predicate _require-match _initial-input read-history)
                 (setq prompt read-prompt
                       history read-history
                       candidates (all-completions "" collection nil)
                       metadata (funcall collection "" nil 'metadata))
                 (cadr candidates))))
      (let* ((node (org-slipbox-ref-read nil nil "Choose ref: "))
             (props (cdr metadata))
             (annotation (funcall (cdr (assq 'annotation-function props))
                                  (cadr candidates))))
        (should (equal method "slipbox/searchRefs"))
        (should (equal params '(:query "" :limit 200)))
        (should (equal prompt "Choose ref: "))
        (should (eq history 'org-slipbox-ref-history))
        (should (= (length candidates) 2))
        (should (equal (plist-get node :title) "Paper B"))
        (should (equal annotation " [Paper B]"))))))

(ert-deftest org-slipbox-test-ref-read-applies-filter-function ()
  "Ref read should filter candidate nodes before completion."
  (let (candidates)
    (cl-letf (((symbol-function 'org-slipbox-rpc-search-refs)
               (lambda (_query _limit)
                 '(:refs [(:reference "@alpha"
                           :node (:title "Alpha" :file_path "alpha.org" :line 1))
                          (:reference "@beta"
                           :node (:title "Beta" :file_path "beta.org" :line 2))])))
              ((symbol-function 'completing-read)
               (lambda (_prompt collection _predicate _require-match _initial-input _history)
                 (setq candidates (all-completions "" collection nil))
                 (car candidates))))
      (let ((node (org-slipbox-ref-read nil
                                        (lambda (node)
                                          (string-prefix-p "B" (plist-get node :title))))))
        (should (= (length candidates) 1))
        (should (equal (plist-get node :title) "Beta"))))))

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
                    :prepend :json-false
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
                    :prepend :json-false
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
                :prepend :json-false
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
                    :prepend :json-false
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
                    :prepend :json-false
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

(ert-deftest org-slipbox-test-capture-finalize-syncs-and-refreshes-live-file-target ()
  "Finalizing capture should save and refresh an open file target buffer."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         draft
         target-buffer
         requests)
    (unwind-protect
        (progn
          (make-directory (file-name-directory target) t)
          (write-region "#+title: Note\n" nil target nil 'silent)
          (setq target-buffer (find-file-noselect target))
          (with-current-buffer target-buffer
            (goto-char (point-max))
            (insert "Local edits\n"))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (push (list request-method request-params) requests)
                       (pcase request-method
                         ("slipbox/indexFile"
                          '(:files_indexed 1 :nodes_indexed 1 :links_indexed 0))
                         ("slipbox/captureTemplate"
                          (with-temp-buffer
                            (insert-file-contents target)
                            (should (string-match-p "Local edits" (buffer-string))))
                          (write-region
                           (concat "#+title: Note\nLocal edits\n"
                                   (plist-get request-params :content)
                                   "\n")
                           nil target nil 'silent)
                          '(:title "Note" :file_path "notes/note.org" :line 1))
                         (_
                          (ert-fail
                           (format "unexpected rpc method %s" request-method)))))))
            (setq draft
                  (org-slipbox--capture-node-at-time
                   "Note"
                   '("d" "default" plain "${title}"
                     :target (file "notes/note.org"))
                   nil
                   (encode-time 0 0 0 7 3 2026)))
            (with-current-buffer draft
              (goto-char (point-max))
              (insert "Captured body")
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p draft))
          (should (equal (mapcar #'car (nreverse requests))
                         '("slipbox/indexFile" "slipbox/captureTemplate")))
          (with-current-buffer target-buffer
            (should-not (buffer-modified-p))
            (should (string-match-p "Local edits" (buffer-string)))
            (should (string-match-p "Captured body" (buffer-string)))))
      (when (buffer-live-p draft)
        (kill-buffer draft))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-finalize-syncs-and-refreshes-live-node-target ()
  "Finalizing capture should save and refresh an open node target buffer."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "project.org" root))
         draft
         target-buffer
         requests)
    (unwind-protect
        (progn
          (write-region "* Parent\n" nil target nil 'silent)
          (setq target-buffer (find-file-noselect target))
          (with-current-buffer target-buffer
            (goto-char (point-max))
            (insert "Local project edits\n"))
          (cl-letf (((symbol-function 'org-slipbox-node-from-id)
                     (lambda (_id) nil))
                    ((symbol-function 'org-slipbox-node-from-title-or-alias)
                     (lambda (_query _nocase)
                       '(:node_key "heading:project.org:1"
                         :title "Parent"
                         :file_path "project.org"
                         :line 1)))
                    ((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (push (list request-method request-params) requests)
                       (pcase request-method
                         ("slipbox/indexFile"
                          '(:files_indexed 1 :nodes_indexed 1 :links_indexed 0))
                         ("slipbox/captureTemplate"
                          (with-temp-buffer
                            (insert-file-contents target)
                            (should (string-match-p "Local project edits" (buffer-string))))
                          (write-region
                           (concat "* Parent\nLocal project edits\n"
                                   (plist-get request-params :content)
                                   "\n")
                           nil target nil 'silent)
                          '(:title "Parent" :file_path "project.org" :line 1))
                         (_
                          (ert-fail
                           (format "unexpected rpc method %s" request-method)))))))
            (setq draft
                  (org-slipbox--capture-node-at-time
                   "Child"
                   '("n" "node item" item "${title}"
                     :target (node "Parent"))
                   nil
                   (encode-time 0 0 0 7 3 2026)))
            (with-current-buffer draft
              (org-slipbox-capture-finalize)))
          (should-not (buffer-live-p draft))
          (should (equal (mapcar #'car (nreverse requests))
                         '("slipbox/indexFile" "slipbox/captureTemplate")))
          (with-current-buffer target-buffer
            (should-not (buffer-modified-p))
            (should (string-match-p "Local project edits" (buffer-string)))
            (should (string-match-p "Child" (buffer-string)))))
      (when (buffer-live-p draft)
        (kill-buffer draft))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-kill-buffer-closes-target-opened-by-finalize ()
  "`:kill-buffer' should close target buffers that were not open before capture."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         opened-buffer)
    (unwind-protect
        (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                   (lambda (_request-method request-params)
                     (let ((target (expand-file-name
                                    (plist-get request-params :file_path)
                                    root)))
                       (make-directory (file-name-directory target) t)
                       (write-region "#+title: Note\n" nil target nil 'silent))
                     '(:title "Note" :file_path "notes/note.org" :line 1)))
                  ((symbol-function 'org-slipbox--visit-node)
                   (lambda (node &optional _other-window)
                     (setq opened-buffer
                           (find-file-noselect
                            (expand-file-name (plist-get node :file_path) root))))))
          (org-slipbox--capture-node-at-time
           "Note"
           '("d" "default" plain "${title}"
             :target (file "notes/note.org")
             :jump-to-captured t
             :kill-buffer t
             :immediate-finish t)
           nil
           (encode-time 0 0 0 7 3 2026))
          (should opened-buffer)
          (should-not (buffer-live-p opened-buffer)))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-kill-buffer-preserves-preexisting-target-buffer ()
  "`:kill-buffer' should not close target buffers that were already open."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         target-buffer
         visited)
    (unwind-protect
        (progn
          (make-directory (file-name-directory target) t)
          (write-region "#+title: Note\n" nil target nil 'silent)
          (setq target-buffer (find-file-noselect target))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_request-method _request-params)
                       '(:title "Note" :file_path "notes/note.org" :line 1)))
                    ((symbol-function 'org-slipbox--visit-node)
                     (lambda (node &optional _other-window)
                       (setq visited node)
                       (find-file-noselect
                        (expand-file-name (plist-get node :file_path) root)))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" plain "${title}"
               :target (file "notes/note.org")
               :jump-to-captured t
               :kill-buffer t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should visited)
          (should (buffer-live-p target-buffer)))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-unnarrowed-visits-full-buffer ()
  "`:unnarrowed' should be accepted and leave the visited buffer widened."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         target-buffer)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_request-method _request-params)
                       (make-directory (file-name-directory target) t)
                       (write-region "* Note\n" nil target nil 'silent)
                       '(:title "Note" :file_path "notes/note.org" :line 1))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :jump-to-captured t
               :unnarrowed t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (setq target-buffer (get-file-buffer target))
          (should (buffer-live-p target-buffer))
          (with-current-buffer target-buffer
            (should-not (buffer-narrowed-p))))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-clock-in-starts-clock-on-captured-node ()
  "`:clock-in' should start an Org clock on the captured node."
  (require 'org-clock)
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         (org-clock-persist nil)
         (org-log-note-clock-out nil))
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_request-method _request-params)
                       (make-directory (file-name-directory target) t)
                       (write-region "* Note\n" nil target nil 'silent)
                       '(:title "Note" :file_path "notes/note.org" :line 1))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :clock-in t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should (org-clocking-p))
          (should (equal org-clock-heading "Note")))
      (when (org-clocking-p)
        (org-clock-out))
      (when-let ((buffer (get-file-buffer target)))
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-clock-resume-restores-previous-clock ()
  "`:clock-resume' should restore the previous clock after capture."
  (require 'org-clock)
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (existing-file (expand-file-name "existing.org" root))
         (target (expand-file-name "notes/note.org" root))
         (org-clock-persist nil)
         (org-log-note-clock-out nil)
         existing-buffer)
    (unwind-protect
        (progn
          (write-region "* Existing\n" nil existing-file nil 'silent)
          (setq existing-buffer (find-file-noselect existing-file))
          (with-current-buffer existing-buffer
            (goto-char (point-min))
            (org-clock-in))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_request-method _request-params)
                       (make-directory (file-name-directory target) t)
                       (write-region "* Note\n" nil target nil 'silent)
                       '(:title "Note" :file_path "notes/note.org" :line 1))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :clock-in t
               :clock-resume t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should (org-clocking-p))
          (should (equal org-clock-heading "Existing")))
      (when (org-clocking-p)
        (org-clock-out))
      (when (buffer-live-p existing-buffer)
        (kill-buffer existing-buffer))
      (when-let ((buffer (get-file-buffer target)))
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-clock-keep-preserves-current-clock ()
  "`:clock-keep' should leave the current clock running."
  (require 'org-clock)
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (existing-file (expand-file-name "existing.org" root))
         (target (expand-file-name "notes/note.org" root))
         (org-clock-persist nil)
         (org-log-note-clock-out nil)
         existing-buffer)
    (unwind-protect
        (progn
          (write-region "* Existing\n" nil existing-file nil 'silent)
          (setq existing-buffer (find-file-noselect existing-file))
          (with-current-buffer existing-buffer
            (goto-char (point-min))
            (org-clock-in))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (_request-method _request-params)
                       (make-directory (file-name-directory target) t)
                       (write-region "* Note\n" nil target nil 'silent)
                       '(:title "Note" :file_path "notes/note.org" :line 1))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :clock-in t
               :clock-keep t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should (org-clocking-p))
          (should (equal org-clock-heading "Existing")))
      (when (org-clocking-p)
        (org-clock-out))
      (when (buffer-live-p existing-buffer)
        (kill-buffer existing-buffer))
      (when-let ((buffer (get-file-buffer target)))
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-no-save-leaves-new-target-buffer-unsaved ()
  "`:no-save' should keep new target buffers unsaved and off disk."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         target-buffer
         methods)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (push (list request-method request-params) methods)
                       (pcase request-method
                         ("slipbox/captureTemplatePreview"
                          '(:file_path "notes/note.org"
                            :content "* Note\n"
                            :preview_node
                            (:title "Note"
                             :file_path "notes/note.org"
                             :line 1
                             :kind heading)))
                         (_
                          (ert-fail
                           (format "unexpected rpc method %s" request-method)))))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :no-save t
               :jump-to-captured t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should (equal (mapcar #'car (nreverse methods))
                         '("slipbox/captureTemplatePreview")))
          (should-not (file-exists-p target))
          (setq target-buffer (get-file-buffer target))
          (should (buffer-live-p target-buffer))
          (with-current-buffer target-buffer
            (should (buffer-modified-p))
            (should (equal (buffer-string) "* Note\n"))))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-no-save-preserves-dirty-live-target-buffer ()
  "`:no-save' should preview against dirty live target buffers without saving them."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         target-buffer
         methods)
    (unwind-protect
        (progn
          (make-directory (file-name-directory target) t)
          (write-region "#+title: Note\n" nil target nil 'silent)
          (setq target-buffer (find-file-noselect target))
          (with-current-buffer target-buffer
            (goto-char (point-max))
            (insert "Local edits\n"))
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (push (list request-method request-params) methods)
                       (pcase request-method
                         ("slipbox/captureTemplatePreview"
                          (should (string-match-p
                                   "Local edits"
                                   (plist-get request-params :source_override)))
                          '(:file_path "notes/note.org"
                            :content "#+title: Note\nLocal edits\nCaptured body\n"
                            :preview_node
                            (:title "Note"
                             :file_path "notes/note.org"
                             :line 1
                             :kind file)))
                         (_
                          (ert-fail
                           (format "unexpected rpc method %s" request-method)))))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" plain "Captured body"
               :target (file "notes/note.org")
               :no-save t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should (equal (mapcar #'car (nreverse methods))
                         '("slipbox/captureTemplatePreview")))
          (should (equal (with-temp-buffer
                           (insert-file-contents target)
                           (buffer-string))
                         "#+title: Note\n"))
          (with-current-buffer target-buffer
            (should (buffer-modified-p))
            (should (string-match-p "Local edits" (buffer-string)))
            (should (string-match-p "Captured body" (buffer-string)))))
      (when (buffer-live-p target-buffer)
        (kill-buffer target-buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-no-save-insert-link-uses-preview-id ()
  "`:no-save' insert-link finalization should use the preview node ID directly."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         methods
         added-location)
    (unwind-protect
        (with-temp-buffer
          (org-mode)
          (let ((marker (point-marker)))
            (cl-letf (((symbol-function 'org-id-add-location)
                       (lambda (id file)
                         (setq added-location (list id file))))
                      ((symbol-function 'org-slipbox-rpc-request)
                       (lambda (request-method request-params)
                         (push (list request-method request-params) methods)
                         (pcase request-method
                           ("slipbox/captureTemplatePreview"
                            (should (plist-get request-params :ensure_node_id))
                            '(:file_path "notes/note.org"
                              :content "* Note\n:PROPERTIES:\n:ID: note-1\n:END:\n"
                              :preview_node
                              (:title "Note"
                               :file_path "notes/note.org"
                               :line 1
                               :kind heading
                               :explicit_id "note-1")))
                           ("slipbox/ensureNodeId"
                            (ert-fail "no-save insert-link should not assign IDs after preview"))
                           (_
                            (ert-fail
                             (format "unexpected rpc method %s" request-method)))))))
              (org-slipbox--capture-node-at-time
               "Note"
               '("d" "default" entry "* ${title}"
                 :target (file "notes/note.org")
                 :no-save t
                 :immediate-finish t)
               nil
               (encode-time 0 0 0 7 3 2026)
               nil
               `(:finalize insert-link
                 :call-location ,marker
                 :link-description "Inserted")))
            (should (equal (buffer-string)
                           "[[id:note-1][Inserted]]"))))
      (should (equal (mapcar #'car (nreverse methods))
                     '("slipbox/captureTemplatePreview")))
      (should
       (equal added-location
              (list "note-1" target)))
      (when-let ((buffer (get-file-buffer target)))
        (kill-buffer buffer))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-capture-no-save-kill-buffer-saves-before-kill ()
  "`:kill-buffer' should force a real save even when `:no-save' is set."
  (let* ((root (make-temp-file "org-slipbox-capture-" t))
         (org-slipbox-directory root)
         (target (expand-file-name "notes/note.org" root))
         saved-params
         opened-buffer)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-capture-template)
                     (lambda (request-params)
                       (setq saved-params request-params)
                       (make-directory (file-name-directory target) t)
                       (write-region "* Note\n" nil target nil 'silent)
                       '(:title "Note" :file_path "notes/note.org" :line 1)))
                    ((symbol-function 'org-slipbox-rpc-capture-template-preview)
                     (lambda (&rest _args)
                       (ert-fail ":kill-buffer + :no-save should not use preview capture")))
                    ((symbol-function 'org-slipbox--visit-node)
                     (lambda (node &optional _other-window)
                       (setq opened-buffer
                             (find-file-noselect
                              (expand-file-name (plist-get node :file_path) root))))))
            (org-slipbox--capture-node-at-time
             "Note"
             '("d" "default" entry "* ${title}"
               :target (file "notes/note.org")
               :no-save t
               :kill-buffer t
               :jump-to-captured t
               :immediate-finish t)
             nil
             (encode-time 0 0 0 7 3 2026)))
          (should saved-params)
          (should (file-exists-p target))
          (should opened-buffer)
          (should-not (buffer-live-p opened-buffer)))
      (delete-directory root t))))

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

(ert-deftest org-slipbox-test-node-from-title-or-alias-encodes-false-nocase ()
  "Exact title lookup should encode false JSON booleans explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 nil)))
      (org-slipbox-node-from-title-or-alias "batman"))
    (should (equal method "slipbox/nodeFromTitleOrAlias"))
    (should (equal params '(:title_or_alias "batman" :nocase :json-false)))))

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

(ert-deftest org-slipbox-test-node-link-occurrence-renderer-uses-related-node-slot ()
  "Link occurrence rendering should use the related node slot structurally."
  (with-temp-buffer
    (org-slipbox-node--insert-link-occurrence
     '(:destination_node (:title "Target heading"
                          :file_path "beta.org"
                          :line 7)
       :row 9
       :col 5
       :preview "See [[id:beta-target][Beta]].")
     :destination_node)
    (let ((contents (buffer-string)))
      (should (string-match-p "Target heading" contents))
      (should (string-match-p "beta.org:9:5" contents))
      (should (string-match-p "See \\[\\[id:beta-target\\]\\[Beta\\]\\]" contents)))))

(ert-deftest org-slipbox-test-node-forward-links-uses-rpc-and-renders-results ()
  "Forward-link command should use the dedicated RPC and render results."
  (let* ((response
          (list :forward_links
                (vector
                 (list :destination_node
                       (list :title "Target heading"
                             :file_path "beta.org"
                             :line 7)
                       :row 9
                       :col 5
                       :preview "See [[id:beta-target][Beta]]."))))
         rpc-args
         rendered)
    (cl-letf (((symbol-function 'org-slipbox-node-read)
               (lambda (&rest _args)
                 '(:node_key "heading:alpha.org:3"
                   :title "Source heading"
                   :file_path "alpha.org"
                   :line 3)))
              ((symbol-function 'org-slipbox-rpc-forward-links)
               (lambda (node-key &optional limit unique)
                 (setq rpc-args (list node-key limit unique))
                 response))
              ((symbol-function 'display-buffer)
               (lambda (buffer-or-name &optional _action _frame)
                 (setq rendered
                       (with-current-buffer (get-buffer buffer-or-name)
                         (buffer-string)))
                 nil)))
      (org-slipbox-node-forward-links))
    (should (equal rpc-args '("heading:alpha.org:3" 200 nil)))
    (should (string-match-p "Forward links for Source heading" rendered))
    (should (string-match-p "Target heading" rendered))
    (should (string-match-p "beta.org:9:5" rendered))
    (should (string-match-p "See \\[\\[id:beta-target\\]\\[Beta\\]\\]" rendered))))

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

(ert-deftest org-slipbox-test-buffer-node-section-renders-indexed-metadata ()
  "Node sections should render indexed metadata when it is present."
  (let* ((mtime-ns 1741353600000000000)
         (expected-mtime
          (format-time-string
           "%Y-%m-%d"
           (seconds-to-time (/ (float mtime-ns) 1000000000.0))))
         (org-slipbox-buffer-sections '(org-slipbox-buffer-node-section)))
    (with-current-buffer (get-buffer-create "*org-slipbox metadata test*")
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        `(:node_key "heading:note.org:3"
                          :title "Heading"
                          :file_path "note.org"
                          :line 3
                          :file_mtime_ns ,mtime-ns
                          :backlink_count 4
                          :forward_link_count 2))
            (org-slipbox-buffer-render-contents)
            (let ((contents (buffer-string)))
              (should (string-match-p (regexp-quote expected-mtime) contents))
              (should (string-match-p "Backlinks:[[:space:]]+4" contents))
              (should (string-match-p "Forward Links:[[:space:]]+2" contents))))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-buffer-render-honors-section-order-and-postrender-hook ()
  "Configured sections should render in order and support postrender hooks."
  (let ((org-slipbox-buffer-sections
         '((org-slipbox-buffer-backlinks-section
            :unique t
            :section-heading "Unique Backlinks")
           org-slipbox-buffer-refs-section))
        (org-slipbox-buffer-postrender-functions
         (list (lambda ()
                 (insert "Postrender marker\n"))))
        rpc-args)
    (with-current-buffer (get-buffer-create "*org-slipbox section test*")
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        '(:node_key "heading:note.org:3"
                          :title "Heading"
                          :file_path "note.org"
                          :line 3
                          :refs ["@smith2024"]))
            (cl-letf (((symbol-function 'org-slipbox-rpc-backlinks)
                       (lambda (node-key &optional limit unique)
                         (setq rpc-args (list node-key limit unique))
                         '(:backlinks [(:source_node (:title "Backlink"
                                                :file_path "other.org"
                                                :line 10)
                                       :row 12
                                       :col 4
                                       :preview "See [[id:heading]].")]))))
              (org-slipbox-buffer-render-contents))
            (let ((contents (buffer-string)))
              (should (< (string-match-p "Unique Backlinks" contents)
                         (string-match-p "Refs" contents)))
              (should (equal rpc-args '("heading:note.org:3" 200 t)))
              (should (string-match-p "Postrender marker" contents))))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-buffer-section-filter-skips-selected-sections ()
  "Section filters should be able to suppress configured sections."
  (let ((org-slipbox-buffer-sections
         '(org-slipbox-buffer-node-section
           org-slipbox-buffer-refs-section
           org-slipbox-buffer-backlinks-section))
        (org-slipbox-buffer-section-filter-function
         (lambda (section _node)
           (not (eq section 'org-slipbox-buffer-refs-section)))))
    (with-current-buffer (get-buffer-create "*org-slipbox filter test*")
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        '(:node_key "heading:note.org:3"
                          :title "Heading"
                          :file_path "note.org"
                          :line 3
                          :refs ["@smith2024"]))
            (cl-letf (((symbol-function 'org-slipbox-rpc-backlinks)
                       (lambda (&rest _args)
                         '(:backlinks []))))
              (org-slipbox-buffer-render-contents))
            (should-not (string-match-p "\nRefs\n----\n" (buffer-string)))
            (should (string-match-p "\nBacklinks\n---------\n" (buffer-string))))
        (kill-buffer (current-buffer))))))

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

(ert-deftest org-slipbox-test-buffer-persistent-mode-owns-hook-lifecycle ()
  "Persistent buffer redisplay should be owned by its explicit mode."
  (let ((org-slipbox-buffer-persistent-mode nil))
    (unwind-protect
        (progn
          (org-slipbox-buffer-persistent-mode 1)
          (should org-slipbox-buffer-persistent-mode)
          (should (memq #'org-slipbox-buffer--redisplay-h post-command-hook))
          (org-slipbox-buffer-persistent-mode -1)
          (should-not org-slipbox-buffer-persistent-mode)
          (should-not (memq #'org-slipbox-buffer--redisplay-h post-command-hook)))
      (when org-slipbox-buffer-persistent-mode
        (org-slipbox-buffer-persistent-mode -1)))))

(ert-deftest org-slipbox-test-graph-write-dot-uses-rpc-and-writes-file ()
  "Graph DOT export should request DOT from the daemon and write it to disk."
  (require 'org-slipbox-graph)
  (let* ((output (make-temp-file "org-slipbox-graph-" nil ".dot"))
         method
         params)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                     (lambda (request-method request-params)
                       (setq method request-method
                             params request-params)
                       '(:dot "digraph \"org-slipbox\" {}\n"))))
            (org-slipbox-graph-write-dot nil nil output))
          (should (equal method "slipbox/graphDot"))
          (should (equal (plist-get params :include_orphans) t))
          (should (equal (plist-get params :hidden_link_types) []))
          (should (equal (plist-get params :shorten_titles) "truncate"))
          (should (equal (plist-get params :node_url_prefix)
                         "org-protocol://roam-node?node="))
          (with-temp-buffer
            (insert-file-contents output)
            (should (equal (buffer-string) "digraph \"org-slipbox\" {}\n"))))
      (when (file-exists-p output)
        (delete-file output)))))

(ert-deftest org-slipbox-test-graph-write-file-renders-neighborhood-graph ()
  "Rendered graph export should request neighborhood DOT and invoke Graphviz."
  (require 'org-slipbox-graph)
  (let* ((output (make-temp-file "org-slipbox-graph-" nil ".svg"))
         rpc-params
         dot-command)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'org-slipbox-rpc-graph-dot)
                     (lambda (params)
                       (setq rpc-params params)
                       '(:dot "digraph \"org-slipbox\" {}\n")))
                    ((symbol-function 'executable-find)
                     (lambda (program)
                       (and (string= program "dot") "/usr/bin/dot")))
                    ((symbol-function 'call-process)
                     (lambda (&rest args)
                       (setq dot-command args)
                       (write-region "<svg/>" nil output nil 'silent)
                       0)))
            (org-slipbox-graph-write-file 2 '(:node_key "file:alpha.org") output))
          (should (equal (plist-get rpc-params :root_node_key) "file:alpha.org"))
          (should (equal (plist-get rpc-params :max_distance) 2))
          (should (eq (plist-get rpc-params :include_orphans) :json-false))
          (should (equal (plist-get rpc-params :hidden_link_types) []))
          (should (equal (car dot-command) "dot"))
          (with-temp-buffer
            (insert-file-contents output)
            (should (equal (buffer-string) "<svg/>"))))
      (when (file-exists-p output)
        (delete-file output)))))

(ert-deftest org-slipbox-test-graph-write-file-runs-generation-hook ()
  "Rendered graph export should run the generation hook on success."
  (require 'org-slipbox-graph)
  (let* ((output (make-temp-file "org-slipbox-graph-" nil ".svg"))
         hook-dot-exists
         hook-dot
         hook-output)
    (unwind-protect
        (let ((org-slipbox-graph-generation-hook
               (list
                (lambda (dot-file graph-file)
                  (setq hook-dot-exists (file-exists-p dot-file)
                        hook-dot (when (file-exists-p dot-file)
                                   (with-temp-buffer
                                     (insert-file-contents dot-file)
                                     (buffer-string)))
                        hook-output graph-file)))))
          (cl-letf (((symbol-function 'org-slipbox-rpc-graph-dot)
                     (lambda (_params)
                       '(:dot "digraph \"org-slipbox\" { \"n\"; }\n")))
                    ((symbol-function 'executable-find)
                     (lambda (program)
                       (and (string= program "dot") "/usr/bin/dot")))
                    ((symbol-function 'call-process)
                     (lambda (&rest _args)
                       (write-region "<svg/>" nil output nil 'silent)
                       0)))
            (org-slipbox-graph-write-file nil nil output))
          (should hook-dot-exists)
          (should (equal hook-dot "digraph \"org-slipbox\" { \"n\"; }\n"))
          (should (equal hook-output output)))
      (when (file-exists-p output)
        (delete-file output)))))

(ert-deftest org-slipbox-test-graph-command-opens-rendered-output ()
  "Interactive graph rendering should open the generated file."
  (require 'org-slipbox-graph)
  (let (build-args opened)
    (cl-letf (((symbol-function 'org-slipbox-graph--build-file)
               (lambda (&rest args)
                 (setq build-args args)
                 (let ((callback (nth 3 args)))
                   (when callback
                     (funcall callback "/tmp/generated.svg")))
                 "/tmp/generated.svg"))
              ((symbol-function 'org-slipbox-graph--open)
               (lambda (file)
                 (setq opened file))))
      (should (equal (org-slipbox-graph '(4) '(:node_key "file:alpha.org"))
                     "/tmp/generated.svg")))
    (should (equal (car build-args) '(4)))
    (should (equal (cadr build-args) '(:node_key "file:alpha.org")))
    (should (equal opened "/tmp/generated.svg"))))

(ert-deftest org-slipbox-test-buffer-reflinks-use-daemon-query ()
  "Reflink discovery should use the dedicated daemon query."
  (let (rpc-args)
    (cl-letf (((symbol-function 'org-slipbox-rpc-reflinks)
               (lambda (node-key &optional limit)
                 (setq rpc-args (list node-key limit))
                 '(:reflinks [(:source_node (:title "Sibling"
                                       :file_path "note.org"
                                       :line 9)
                              :row 10
                              :col 3
                              :preview "cite:smith2024"
                              :matched_reference "cite:smith2024")])))
              ((symbol-function 'executable-find)
               (lambda (&rest _args)
                 (ert-fail "reflinks should not depend on rg")))
              ((symbol-function 'shell-command-to-string)
               (lambda (&rest _args)
                 (ert-fail "reflinks should not shell out"))))
      (should
       (equal
         (org-slipbox-buffer--reflinks
         '(:node_key "heading:note.org:3" :refs ["@smith2024"]))
        '((:source_node (:title "Sibling"
                    :file_path "note.org"
                    :line 9)
           :row 10
           :col 3
           :preview "cite:smith2024"
           :matched_reference "cite:smith2024")))))
    (should (equal rpc-args '("heading:note.org:3" 200))))

(ert-deftest org-slipbox-test-buffer-forward-links-use-daemon-query ()
  "Forward-link discovery should use the dedicated daemon query."
  (let (rpc-args)
    (cl-letf (((symbol-function 'org-slipbox-rpc-forward-links)
               (lambda (node-key &optional limit unique)
                 (setq rpc-args (list node-key limit unique))
                 '(:forward_links
                   [(:destination_node (:title "Target heading"
                                       :file_path "target.org"
                                       :line 12)
                     :row 8
                     :col 5
                     :preview "[[id:target][Target heading]]")]))))
      (let ((expected
             '((:destination_node (:title "Target heading"
                                  :file_path "target.org"
                                  :line 12)
                :row 8
                :col 5
                :preview "[[id:target][Target heading]]"))))
        (should
         (equal
          (org-slipbox-buffer--forward-links
           '(:node_key "heading:note.org:3"))
          expected))))
    (should (equal rpc-args '("heading:note.org:3" 200 nil)))))

(ert-deftest org-slipbox-test-buffer-unlinked-references-use-daemon-query ()
  "Unlinked discovery should use the dedicated daemon query."
  (let (rpc-args)
    (cl-letf (((symbol-function 'org-slipbox-rpc-unlinked-references)
               (lambda (node-key &optional limit)
                 (setq rpc-args (list node-key limit))
                 '(:unlinked_references
                   [(:source_node (:title "Sibling"
                                  :file_path "note.org"
                                  :line 9)
                     :row 10
                     :col 3
                     :preview "Project Atlas should surface."
                     :matched_text "Project Atlas")]))))
              ((symbol-function 'executable-find)
               (lambda (&rest _args)
                 (ert-fail "unlinked references should not depend on rg")))
              ((symbol-function 'shell-command-to-string)
               (lambda (&rest _args)
                 (ert-fail "unlinked references should not shell out"))))
      (let ((expected
             '((:source_node (:title "Sibling"
                             :file_path "note.org"
                             :line 9)
                :row 10
                :col 3
                :preview "Project Atlas should surface."
                :matched_text "Project Atlas"))))
        (should
         (equal
          (org-slipbox-buffer--unlinked-references
           '(:node_key "heading:note.org:3"
             :title "Project Atlas"
             :aliases ["Atlas Plan"]))
          expected))))
    (should (equal rpc-args '("heading:note.org:3" 200)))))

(ert-deftest org-slipbox-test-buffer-dedicated-render-includes-discovery-sections ()
  "Dedicated buffers should render expensive discovery sections by default."
  (let* ((mtime-ns 1741353600000000000)
         (expected-mtime
          (format-time-string
           "%Y-%m-%d"
           (seconds-to-time (/ (float mtime-ns) 1000000000.0))))
         (org-slipbox-directory "/tmp")
         (org-slipbox-buffer-expensive-sections 'dedicated))
    (with-current-buffer (get-buffer-create "*org-slipbox: Note<note.org>*")
      (unwind-protect
          (progn
            (setq-local org-slipbox-buffer-current-node
                        `(:node_key "file:note.org"
                          :title "Note"
                          :file_path "note.org"
                          :line 1
                          :kind "file"
                          :file_mtime_ns ,mtime-ns
                          :backlink_count 3
                          :forward_link_count 2))
            (cl-letf (((symbol-function 'org-slipbox-buffer--backlinks)
                       (lambda (&rest _args) nil))
                      ((symbol-function 'org-slipbox-buffer--forward-links)
                       (lambda (&rest _args)
                         '((:destination_node (:title "Target"
                                         :file_path "target.org"
                                         :line 11)
                            :row 4
                            :col 5
                            :preview "[[id:target][Target]]"))))
                      ((symbol-function 'org-slipbox-buffer--reflinks)
                       (lambda (_node)
                         '((:source_node (:title "Sibling"
                                      :file_path "refs.org"
                                      :line 2)
                            :row 3
                            :col 7
                            :preview "cite:smith2024"
                            :matched_reference "cite:smith2024"))))
                      ((symbol-function 'org-slipbox-buffer--unlinked-references)
                       (lambda (_node)
                         '((:source_node (:title "Atlas"
                                      :file_path "unlinked.org"
                                      :line 8)
                            :row 9
                            :col 2
                            :preview "Note mention"
                            :matched_text "Note")))))
              (org-slipbox-buffer-render-contents))
            (should (string-match-p (regexp-quote expected-mtime) (buffer-string)))
            (should (string-match-p "Backlinks:[[:space:]]+3" (buffer-string)))
            (should (string-match-p "Forward Links:[[:space:]]+2" (buffer-string)))
            (should (string-match-p "Forward Links" (buffer-string)))
            (should (string-match-p "Reflinks" (buffer-string)))
            (should (string-match-p "Unlinked References" (buffer-string)))
            (should (string-match-p "Target" (buffer-string)))
            (should (string-match-p "Sibling" (buffer-string)))
            (should (string-match-p "Atlas" (buffer-string)))
            (should (string-match-p "cite:smith2024" (buffer-string)))
            (should (string-match-p "Note mention" (buffer-string))))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-buffer-persistent-render-skips-expensive-sections ()
  "Persistent buffers should skip expensive discovery sections by default."
  (let ((org-slipbox-directory "/tmp")
        (org-slipbox-buffer-expensive-sections 'dedicated)
        expensive-called
        forward-called)
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
                       (lambda (&rest _args) nil))
                      ((symbol-function 'org-slipbox-buffer--forward-links)
                       (lambda (&rest _args)
                         (setq forward-called t)
                         nil))
                      ((symbol-function 'org-slipbox-buffer--reflinks)
                       (lambda (_node)
                         (setq expensive-called t)
                         nil))
                      ((symbol-function 'org-slipbox-buffer--unlinked-references)
                       (lambda (_node)
                         (setq expensive-called t)
                         nil)))
              (org-slipbox-buffer-render-contents))
            (should forward-called)
            (should-not expensive-called))
        (kill-buffer (current-buffer))))))

(ert-deftest org-slipbox-test-diagnose-node-renders-status-file-and-node ()
  "Node diagnostics should include daemon status, file state, and node data."
  (let* ((root (make-temp-file "org-slipbox-diagnose-" t))
         (file (expand-file-name "note.org" root))
         (buffer-name "*org-slipbox diagnostics*"))
    (unwind-protect
        (progn
          (write-region "#+title: Note\n" nil file nil 'silent)
          (with-current-buffer (find-file-noselect file)
            (let ((org-slipbox-directory root)
                  (org-slipbox-autosync-mode t))
              (cl-letf (((symbol-function 'org-slipbox-rpc-status)
                         (lambda ()
                           '(:version "0.1.0"
                             :root "/tmp/root"
                             :db "/tmp/org-slipbox.sqlite"
                             :files_indexed 2
                             :nodes_indexed 3
                             :links_indexed 4)))
                        ((symbol-function 'org-slipbox-rpc-indexed-files)
                         (lambda ()
                           '(:files ["note.org"])))
                        ((symbol-function 'org-slipbox-node-at-point)
                         (lambda (&optional _assert)
                           '(:title "Heading"
                             :file_path "note.org"
                             :line 2
                             :node_key "heading:note.org:2"))))
                (org-slipbox-diagnose-node)))
            (kill-buffer (current-buffer)))
          (with-current-buffer buffer-name
            (should (string-match-p "Status" (buffer-string)))
            (should (string-match-p "Autosync:[[:space:]]+enabled" (buffer-string)))
            (should (string-match-p "Eligible:[[:space:]]+yes" (buffer-string)))
            (should (string-match-p "Indexed:[[:space:]]+yes" (buffer-string)))
            (should (string-match-p ":title \"Heading\"" (buffer-string)))))
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-diagnose-file-renders-eligibility-reasons ()
  "File diagnostics should explain exclusion and eligibility state."
  (let* ((root (make-temp-file "org-slipbox-diagnose-" t))
         (archive (expand-file-name "archive" root))
         (file (expand-file-name "skip.org" archive))
         (buffer-name "*org-slipbox file diagnostics*"))
    (unwind-protect
        (progn
          (make-directory archive t)
          (write-region "" nil file nil 'silent)
          (let ((org-slipbox-directory root)
                (org-slipbox-file-exclude-regexp "^archive/"))
            (cl-letf (((symbol-function 'org-slipbox-rpc-indexed-files)
                       (lambda ()
                         '(:files []))))
              (org-slipbox-diagnose-file file)))
          (with-current-buffer buffer-name
            (should (string-match-p "Excluded by policy:[[:space:]]+yes" (buffer-string)))
            (should (string-match-p "Matched excludes:[[:space:]]+\\^archive/" (buffer-string)))
            (should (string-match-p "Eligible:[[:space:]]+no" (buffer-string)))))
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-list-files-report-compares-eligible-and-indexed ()
  "File reports should expose drift between eligible and indexed files."
  (let* ((root (make-temp-file "org-slipbox-files-report-" t))
         (keep (expand-file-name "keep.org" root))
         (missing (expand-file-name "missing.org" root))
         (buffer-name "*org-slipbox files*"))
    (unwind-protect
        (progn
          (write-region "" nil keep nil 'silent)
          (write-region "" nil missing nil 'silent)
          (let ((org-slipbox-directory root))
            (cl-letf (((symbol-function 'org-slipbox-rpc-status)
                       (lambda ()
                         '(:files_indexed 2 :nodes_indexed 2 :links_indexed 0)))
                      ((symbol-function 'org-slipbox-rpc-indexed-files)
                       (lambda ()
                         '(:files ["keep.org" "stale.org"]))))
              (org-slipbox-list-files-report)))
          (with-current-buffer buffer-name
            (should (string-match-p "Eligible But Not Indexed" (buffer-string)))
            (should (string-match-p "^missing\\.org$" (buffer-string)))
            (should (string-match-p "Indexed But Not Eligible" (buffer-string)))
            (should (string-match-p "^stale\\.org$" (buffer-string)))))
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name))
      (delete-directory root t))))

(ert-deftest org-slipbox-test-db-explore-opens-configured-sqlite-file ()
  "Database exploration should open the SQLite file reported by the daemon."
  (let ((original-require (symbol-function 'require))
        opened)
    (cl-letf (((symbol-function 'org-slipbox-rpc-status)
               (lambda ()
                 '(:db "/tmp/org-slipbox.sqlite")))
              ((symbol-function 'require)
               (lambda (feature &optional _filename _noerror)
                 (unless (eq feature 'sqlite-mode)
                   (funcall original-require feature))))
              ((symbol-function 'sqlite-mode-open-file)
               (lambda (file)
                 (setq opened file))))
      (org-slipbox-db-explore))
    (should (equal opened "/tmp/org-slipbox.sqlite"))))

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

(ert-deftest org-slipbox-test-rpc-search-nodes-encodes-sort-param ()
  "Node search RPC should encode named sort modes explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:nodes []))))
      (org-slipbox-rpc-search-nodes "Heading" 10 'forward-link-count))
    (should (equal method "slipbox/searchNodes"))
    (should (equal params '(:query "Heading"
                            :limit 10
                            :sort "forward-link-count")))))

(ert-deftest org-slipbox-test-rpc-search-nodes-rejects-unsupported-string-sort ()
  "Node search RPC should reject unsupported string sort names."
  (should-error
   (org-slipbox-rpc-search-nodes "Heading" 10 "file-atime")
   :type 'user-error))

(ert-deftest org-slipbox-test-rpc-search-files-encodes-params ()
  "File search RPC should encode query and limit explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:files []))))
      (org-slipbox-rpc-search-files "beta" 25))
    (should (equal method "slipbox/searchFiles"))
    (should (equal params '(:query "beta" :limit 25)))))

(ert-deftest org-slipbox-test-rpc-search-occurrences-encodes-params ()
  "Occurrence search RPC should encode query and limit explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:occurrences []))))
      (org-slipbox-rpc-search-occurrences "needle" 25))
    (should (equal method "slipbox/searchOccurrences"))
    (should (equal params '(:query "needle" :limit 25)))))

(ert-deftest org-slipbox-test-rpc-forward-links-encodes-params ()
  "Forward-link RPC should encode limit and unique parameters explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:forward_links []))))
      (org-slipbox-rpc-forward-links "heading:alpha.org:3" 25 t))
    (should (equal method "slipbox/forwardLinks"))
    (should (equal params '(:node_key "heading:alpha.org:3"
                            :limit 25
                            :unique t)))))

(ert-deftest org-slipbox-test-rpc-reflinks-encodes-params ()
  "Reflink RPC should encode node key and limit explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:reflinks []))))
      (org-slipbox-rpc-reflinks "heading:alpha.org:3" 25))
    (should (equal method "slipbox/reflinks"))
    (should (equal params '(:node_key "heading:alpha.org:3"
                            :limit 25)))))

(ert-deftest org-slipbox-test-rpc-unlinked-references-encodes-params ()
  "Unlinked-reference RPC should encode node key and limit explicitly."
  (let (method params)
    (cl-letf (((symbol-function 'org-slipbox-rpc-request)
               (lambda (request-method request-params)
                 (setq method request-method
                       params request-params)
                 '(:unlinked_references []))))
      (org-slipbox-rpc-unlinked-references "heading:alpha.org:3" 25))
    (should (equal method "slipbox/unlinkedReferences"))
    (should (equal params '(:node_key "heading:alpha.org:3"
                            :limit 25)))))

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

(ert-deftest org-slipbox-test-ref-find-uses-ref-read-and-visits-node ()
  "Ref lookup should use `org-slipbox-ref-read' and visit the result."
  (let (read-args visited)
    (cl-letf (((symbol-function 'org-slipbox-ref-read)
               (lambda (&rest args)
                 (setq read-args args)
                 '(:title "Paper" :file_path "paper.org" :line 1)))
              ((symbol-function 'org-slipbox--visit-node)
               (lambda (node)
                 (setq visited node))))
      (org-slipbox-ref-find "smith" #'identity "Lookup ref: "))
    (should (equal (car read-args) "smith"))
    (should (eq (cadr read-args) #'identity))
    (should (equal (caddr read-args) "Lookup ref: "))
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

(ert-deftest org-slipbox-test-global-mode-manages-completion-only-for-eligible-buffers ()
  "The recommended setup mode should enable completion only in eligible Org files."
  (let* ((root (make-temp-file "org-slipbox-mode-" t))
         (inside (expand-file-name "note.org" root))
         (outside-root (make-temp-file "org-slipbox-mode-outside-" t))
         (outside (expand-file-name "other.org" outside-root))
         inside-buffer
         outside-buffer)
    (unwind-protect
        (progn
          (write-region "* Inside\n" nil inside nil 'silent)
          (write-region "* Outside\n" nil outside nil 'silent)
          (setq inside-buffer (find-file-noselect inside)
                outside-buffer (find-file-noselect outside))
          (let ((org-slipbox-directory root)
                (org-slipbox-mode nil)
                (org-slipbox-autosync-mode nil)
                (org-slipbox-id-mode nil))
            (unwind-protect
                (progn
                  (org-slipbox-mode 1)
                  (with-current-buffer inside-buffer
                    (should org-slipbox-completion-mode)
                    (should org-slipbox-mode--managed-completion))
                  (with-current-buffer outside-buffer
                    (should-not org-slipbox-completion-mode)
                    (should-not org-slipbox-mode--managed-completion))
                  (org-slipbox-mode -1)
                  (with-current-buffer inside-buffer
                    (should-not org-slipbox-completion-mode)
                    (should-not org-slipbox-mode--managed-completion)))
              (when org-slipbox-mode
                (org-slipbox-mode -1))
              (when org-slipbox-autosync-mode
                (org-slipbox-autosync-mode -1))
              (when org-slipbox-id-mode
                (org-slipbox-id-mode -1)))))
      (when (buffer-live-p inside-buffer)
        (kill-buffer inside-buffer))
      (when (buffer-live-p outside-buffer)
        (kill-buffer outside-buffer))
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

(ert-deftest org-slipbox-test-dailies-map-exposes-public-prefix-bindings ()
  "Dailies should export a public prefix keymap with stable bindings."
  (should (keymapp org-slipbox-dailies-map))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "d"))
              #'org-slipbox-dailies-goto-today))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "y"))
              #'org-slipbox-dailies-goto-yesterday))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "t"))
              #'org-slipbox-dailies-goto-tomorrow))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "n"))
              #'org-slipbox-dailies-capture-today))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "f"))
              #'org-slipbox-dailies-goto-next-note))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "b"))
              #'org-slipbox-dailies-goto-previous-note))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "c"))
              #'org-slipbox-dailies-goto-date))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "v"))
              #'org-slipbox-dailies-capture-date))
  (should (eq (lookup-key org-slipbox-dailies-map (kbd "."))
              #'org-slipbox-dailies-find-directory)))

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
                    :prepend :json-false
                    :empty_lines_before 0
                    :empty_lines_after 0
                    :file_path "daily/2026-03-07.org"
                    :head "#+title: 2026-03-07"
                    :outline_path ("Inbox"))))
          (should (equal visited '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 8)))
          (should hook-ran))
      (when (buffer-live-p buffer)
        (kill-buffer buffer)))))

(ert-deftest org-slipbox-test-capture-template-title-detection-supports-fixed-dailies ()
  "Capture template title detection should distinguish fixed daily entries."
  (should
   (org-slipbox--capture-template-uses-title-p
    '("d" "default" entry "* ${title}"
      :target (file+head "%<%Y-%m-%d>.org"
                         "#+title: %<%Y-%m-%d>\n"))))
  (should-not
   (org-slipbox--capture-template-uses-title-p
    '("m" "morning" entry
      "* Morning Review\n\n** What am I grateful for?\n"
      :target (file+head "%<%Y-%m-%d>.org"
                         "#+title: %<%Y-%m-%d>\n"))))
  (should
   (org-slipbox--capture-template-uses-title-p
    '("d" "default"
      :path "daily/%<%Y-%m-%d>.org")))
  (should-not
   (org-slipbox--capture-template-uses-title-p
    '("m" "morning"
      :path "daily/%<%Y-%m-%d>.org"
      :title "%<%Y-%m-%d>"))))

(ert-deftest org-slipbox-test-dailies-capture-allows-fixed-template-with-empty-heading ()
  "Fixed-content daily templates should not require a synthetic heading prompt."
  (let (captured-title captured-template captured-time)
    (cl-letf (((symbol-function 'org-slipbox--capture-node-at-time)
               (lambda (title template _refs time _variables _session)
                 (setq captured-title title
                       captured-template template
                       captured-time time)
                 '(:title "Morning Review"))))
      (let ((org-slipbox-dailies-capture-templates
             '(("m" "morning" entry
                "* Morning Review\n\n** What am I grateful for?\n"
                :target (file+head "%<%Y-%m-%d>.org"
                                   "#+title: %<%Y-%m-%d>\n")))))
        (should (equal (org-slipbox-dailies--capture
                        (encode-time 0 0 0 7 3 2026)
                        ""
                        "m")
                       '(:title "Morning Review")))))
    (should (equal captured-title "morning"))
    (should (equal (car captured-template) "m"))
    (should (equal captured-time (encode-time 0 0 0 7 3 2026)))))

(ert-deftest org-slipbox-test-dailies-capture-rejects-empty-heading-for-title-template ()
  "Title-driven daily templates should still require a non-empty heading."
  (let ((org-slipbox-dailies-capture-templates
         '(("d" "default" entry
            "* ${title}"
            :target (file+head "%<%Y-%m-%d>.org"
                               "#+title: %<%Y-%m-%d>\n")))))
    (should-error
     (org-slipbox-dailies--capture
      (encode-time 0 0 0 7 3 2026)
      ""
      "d")
     :type 'user-error)))

(ert-deftest org-slipbox-test-dailies-capture-today-skips-heading-prompt-for-fixed-template ()
  "Interactive dailies capture should skip heading prompts for fixed templates."
  (let ((org-slipbox-dailies-capture-templates
         '(("m" "morning" entry
            "* Morning Review"
            :target (file+head "%<%Y-%m-%d>.org"
                               "#+title: %<%Y-%m-%d>\n"))))
        captured-heading
        captured-key)
    (cl-letf (((symbol-function 'org-slipbox--read-capture-template)
               (lambda (_templates &optional _keys)
                 '("m" "morning" entry
                   "* Morning Review"
                   :target (file+head "%<%Y-%m-%d>.org"
                                      "#+title: %<%Y-%m-%d>\n"))))
              ((symbol-function 'read-string)
               (lambda (&rest _args)
                 (ert-fail "fixed daily templates should not prompt for a heading")))
              ((symbol-function 'org-slipbox-dailies--capture)
               (lambda (_time heading &optional key)
                 (setq captured-heading heading
                       captured-key key)
                 'ok)))
      (should (eq (call-interactively #'org-slipbox-dailies-capture-today) 'ok)))
    (should-not captured-heading)
    (should (equal captured-key "m"))))

(ert-deftest org-slipbox-test-dailies-capture-date-skips-heading-prompt-for-fixed-template ()
  "Date-selected dailies capture should share the fixed-template prompt behavior."
  (let ((org-slipbox-dailies-capture-templates
         '(("e" "evening" entry
            "* Evening Review"
            :target (file+head "%<%Y-%m-%d>.org"
                               "#+title: %<%Y-%m-%d>\n"))))
        captured-heading
        captured-key)
    (cl-letf (((symbol-function 'org-slipbox--read-capture-template)
               (lambda (_templates &optional _keys)
                 '("e" "evening" entry
                   "* Evening Review"
                   :target (file+head "%<%Y-%m-%d>.org"
                                      "#+title: %<%Y-%m-%d>\n"))))
              ((symbol-function 'read-string)
               (lambda (&rest _args)
                 (ert-fail "fixed daily templates should not prompt for a heading")))
              ((symbol-function 'org-slipbox-dailies--read-date)
               (lambda (_prompt _prefer-future)
                 (encode-time 0 0 0 7 3 2026)))
              ((symbol-function 'org-slipbox-dailies--capture)
               (lambda (_time heading &optional key)
                 (setq captured-heading heading
                       captured-key key)
                 'ok)))
      (should (eq (call-interactively #'org-slipbox-dailies-capture-date) 'ok)))
    (should-not captured-heading)
    (should (equal captured-key "e"))))

(ert-deftest org-slipbox-test-dailies-capture-today-prompts-for-title-template ()
  "Interactive dailies capture should still prompt for title-driven templates."
  (let ((org-slipbox-dailies-capture-templates
         '(("d" "default" entry
            "* ${title}"
            :target (file+head "%<%Y-%m-%d>.org"
                               "#+title: %<%Y-%m-%d>\n"))))
        captured-heading
        captured-key
        prompt)
    (cl-letf (((symbol-function 'org-slipbox--read-capture-template)
               (lambda (_templates &optional _keys)
                 '("d" "default" entry
                   "* ${title}"
                   :target (file+head "%<%Y-%m-%d>.org"
                                      "#+title: %<%Y-%m-%d>\n"))))
              ((symbol-function 'read-string)
               (lambda (text &rest _args)
                 (setq prompt text)
                 "Meeting"))
              ((symbol-function 'org-slipbox-dailies--capture)
               (lambda (_time heading &optional key)
                 (setq captured-heading heading
                       captured-key key)
                 'ok)))
      (should (eq (call-interactively #'org-slipbox-dailies-capture-today) 'ok)))
    (should (equal prompt "Daily entry: "))
    (should (equal captured-heading "Meeting"))
    (should (equal captured-key "d"))))

(ert-deftest org-slipbox-test-dailies-template-targets-use-dailies-directory ()
  "Daily templates should root file targets in `org-slipbox-dailies-directory'."
  (let (buffer method params)
    (unwind-protect
        (cl-letf (((symbol-function 'org-slipbox-rpc-request)
                   (lambda (request-method request-params)
                     (setq method request-method
                           params request-params)
                     '(:title "Meeting" :file_path "daily/2026-03-07.org" :line 8)))
                  ((symbol-function 'org-slipbox--visit-node)
                   #'ignore))
          (let ((org-slipbox-dailies-directory "daily/")
                (org-slipbox-dailies-capture-templates
                 '(("d" "default" entry
                    "* %?"
                    :target (file+head "%<%Y-%m-%d>.org"
                                       "#+title: %<%Y-%m-%d>\n")))))
            (setq buffer
                  (org-slipbox-dailies--capture
                   (encode-time 0 0 0 7 3 2026)
                   "Meeting"
                   "d"))
            (with-current-buffer buffer
              (goto-char (point-max))
              (insert "Agenda review")
              (org-slipbox-capture-finalize)))
          (should (equal method "slipbox/captureTemplate"))
          (should (equal (plist-get params :file_path) "daily/2026-03-07.org")))
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
