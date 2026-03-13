;;; org-slipbox-buffer-bench.el --- Batch benchmark for persistent buffer -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.1.0
;; Package-Requires: ((emacs "29.1") (jsonrpc "1.0.27"))
;; Keywords: outlines, files, convenience

;; This file is not part of GNU Emacs.

;;; Commentary:

;; Batch benchmark helpers for the persistent org-slipbox context buffer path.

;;; Code:

(require 'benchmark)
(require 'json)
(require 'org-slipbox-buffer)

(defun org-slipbox-buffer-bench-run-file (fixture-file samples iterations)
  "Return JSON benchmark data for FIXTURE-FILE.
SAMPLES is the number of independent timing samples to collect and
ITERATIONS is the number of redisplay runs per sample."
  (let* ((fixture (with-temp-buffer
                    (insert-file-contents fixture-file)
                    (json-parse-buffer :object-type 'plist :array-type 'list)))
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
              (setq-local org-slipbox-buffer-current-node nil)
              (push
               (* 1000.0
                  (/ (car (benchmark-run-compiled iterations
                            (setq-local org-slipbox-buffer-current-node nil)
                            (org-slipbox-buffer-persistent-redisplay)))
                     iterations))
               samples-ms))))
      (when (get-buffer buffer-name)
        (kill-buffer buffer-name)))
    (json-encode `((samples_ms . ,(nreverse samples-ms))))))

(provide 'org-slipbox-buffer-bench)

;;; org-slipbox-buffer-bench.el ends here
