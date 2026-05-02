;;; org-slipbox-buffer-bench.el --- Batch benchmarks for org-slipbox buffers -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.6.0
;; Package-Requires: ((emacs "29.1") (jsonrpc "1.0.27"))
;; Keywords: outlines, files, convenience

;; This file is not part of GNU Emacs.

;;; Commentary:

;; Batch benchmark helpers for org-slipbox buffer rendering paths.

;;; Code:

(require 'benchmark)
(require 'json)
(require 'org-slipbox-buffer)

(defvar org-slipbox-buffer-bench--dedicated-fixture nil
  "Private fixture used by compiled dedicated-buffer benchmarks.")

(defvar org-slipbox-buffer-bench--exploration-fixture nil
  "Private fixture used by compiled dedicated exploration benchmarks.")

(defun org-slipbox-buffer-bench--read-fixture (fixture-file)
  "Decode benchmark FIXTURE-FILE into a plist."
  (with-temp-buffer
    (insert-file-contents fixture-file)
    (json-parse-buffer :object-type 'plist :array-type 'list)))

(defun org-slipbox-buffer-bench--render-dedicated-comparison ()
  "Render one dedicated comparison state from the private bench fixture."
  (let* ((fixture org-slipbox-buffer-bench--dedicated-fixture)
         (node (plist-get fixture :node))
         (compare-target (plist-get fixture :compare_target))
         (comparison-result (plist-get fixture :comparison_result)))
    (cl-letf (((symbol-function 'org-slipbox-rpc-compare-notes)
               (lambda (_left-node-key _right-node-key &optional _limit)
                 comparison-result)))
      (setq-local
       org-slipbox-buffer-session
       (make-org-slipbox-buffer-session
        :kind 'dedicated
        :current-node node
        :root-node node
        :compare-target compare-target
        :comparison-group 'all
        :trail (list (list :current-node node
                           :active-lens 'structure)
                     (list :current-node node
                           :compare-target compare-target
                           :comparison-group 'all))
        :trail-index 1
        :frozen-context t))
      (org-slipbox-buffer-render-contents))))

(defun org-slipbox-buffer-bench--exploration-branch-base-lens (lens)
  "Return a contrasting branch-base lens for LENS."
  (pcase lens
    ('unresolved 'bridges)
    ('bridges 'unresolved)
    ('dormant 'bridges)
    (_ 'structure)))

(defun org-slipbox-buffer-bench--render-dedicated-exploration ()
  "Render one dedicated exploration state from the private bench fixture."
  (let* ((fixture org-slipbox-buffer-bench--exploration-fixture)
         (node (plist-get fixture :node))
         (lens (intern (plist-get fixture :lens)))
         (exploration-result (plist-get fixture :exploration_result))
         (branch-base-lens (org-slipbox-buffer-bench--exploration-branch-base-lens lens)))
    (cl-letf (((symbol-function 'org-slipbox-rpc-explore)
               (lambda (_node-key requested-lens &optional _limit _unique)
                 (if (eq requested-lens lens)
                     exploration-result
                   `(:lens ,(symbol-name requested-lens) :sections [])))))
      (setq-local
       org-slipbox-buffer-session
       (make-org-slipbox-buffer-session
        :kind 'dedicated
        :current-node node
        :root-node node
        :active-lens lens
        :trail (list (list :current-node node
                           :root-node node
                           :active-lens branch-base-lens
                           :frozen-context t))
        :trail-index 0
        :frozen-context t))
      (org-slipbox-buffer-render-contents))))

(defun org-slipbox-buffer-bench-run-file (fixture-file samples iterations)
  "Return persistent-buffer JSON benchmark data for FIXTURE-FILE.
SAMPLES is the number of independent timing samples to collect and
ITERATIONS is the number of redisplay runs per sample."
  (let* ((fixture (org-slipbox-buffer-bench--read-fixture fixture-file))
         (node (plist-get fixture :node))
         (backlinks (plist-get fixture :backlinks))
         (forward-links (plist-get fixture :forward_links))
         (buffer-name "*org-slipbox-bench*")
         (org-slipbox-buffer buffer-name)
         (org-slipbox-buffer-expensive-sections nil)
         (org-slipbox-buffer-postrender-functions nil)
         (org-slipbox-buffer-section-filter-function nil)
         samples-ms)
    (unwind-protect
        (cl-letf (((symbol-function 'org-slipbox-node-at-point)
                   (lambda () node))
                  ((symbol-function 'org-slipbox-buffer--backlinks)
                   (lambda (_node &optional _unique _limit) backlinks))
                  ((symbol-function 'org-slipbox-buffer--forward-links)
                   (lambda (_node &optional _unique _limit) forward-links)))
          (dotimes (_ samples)
            (with-current-buffer (get-buffer-create buffer-name)
              (setq-local org-slipbox-buffer-session nil)
              (push
               (* 1000.0
                  (/ (car (benchmark-run-compiled iterations
                            (setq-local org-slipbox-buffer-session nil)
                            (org-slipbox-buffer-persistent-redisplay)))
                     iterations))
               samples-ms))))
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name)))
    (json-encode `((samples_ms . ,(nreverse samples-ms))))))

(defun org-slipbox-buffer-bench-run-dedicated-file (fixture-file samples iterations)
  "Return dedicated-buffer JSON benchmark data for FIXTURE-FILE.
SAMPLES is the number of independent timing samples to collect and
ITERATIONS is the number of dedicated renders per sample."
  (let* ((fixture (org-slipbox-buffer-bench--read-fixture fixture-file))
         (buffer-name "*org-slipbox-dedicated-bench*")
         (org-slipbox-buffer-postrender-functions nil)
         (org-slipbox-buffer-section-filter-function nil)
         samples-ms)
    (unwind-protect
        (dotimes (_ samples)
          (with-current-buffer (get-buffer-create buffer-name)
            (setq org-slipbox-buffer-bench--dedicated-fixture fixture)
            (push
             (* 1000.0
                (/ (car (benchmark-run-compiled iterations
                          (org-slipbox-buffer-bench--render-dedicated-comparison)))
                   iterations))
             samples-ms)))
      (setq org-slipbox-buffer-bench--dedicated-fixture nil)
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name)))
    (json-encode `((samples_ms . ,(nreverse samples-ms))))))

(defun org-slipbox-buffer-bench-run-exploration-file (fixture-file samples iterations)
  "Return dedicated-exploration JSON benchmark data for FIXTURE-FILE.
SAMPLES is the number of independent timing samples to collect and
ITERATIONS is the number of dedicated exploration renders per sample."
  (let* ((fixture (org-slipbox-buffer-bench--read-fixture fixture-file))
         (buffer-name "*org-slipbox-exploration-bench*")
         (org-slipbox-buffer-postrender-functions nil)
         (org-slipbox-buffer-section-filter-function nil)
         samples-ms)
    (unwind-protect
        (dotimes (_ samples)
          (with-current-buffer (get-buffer-create buffer-name)
            (setq org-slipbox-buffer-bench--exploration-fixture fixture)
            (push
             (* 1000.0
                (/ (car (benchmark-run-compiled iterations
                          (org-slipbox-buffer-bench--render-dedicated-exploration)))
                   iterations))
             samples-ms)))
      (setq org-slipbox-buffer-bench--exploration-fixture nil)
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name)))
    (json-encode `((samples_ms . ,(nreverse samples-ms))))))

(provide 'org-slipbox-buffer-bench)

;;; org-slipbox-buffer-bench.el ends here
