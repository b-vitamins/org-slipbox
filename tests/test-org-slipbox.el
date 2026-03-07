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

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
