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

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
