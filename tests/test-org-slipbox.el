;;; test-org-slipbox.el --- Tests for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;;; Commentary:

;; Basic smoke tests for the Elisp package.

;;; Code:

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

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
