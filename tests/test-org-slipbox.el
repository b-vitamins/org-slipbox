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

(provide 'test-org-slipbox)

;;; test-org-slipbox.el ends here
