;;; org-slipbox-buffer-query.el --- Query helpers for org-slipbox context buffers -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.18.0
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

;; Query helpers for org-slipbox context buffers.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'org-slipbox-buffer-state)
(require 'org-slipbox-rpc)

(defun org-slipbox-buffer--forward-links (node &optional unique limit)
  "Return forward links for NODE.
When UNIQUE is non-nil, only return the first occurrence per destination
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'forward-links unique limit))

(defun org-slipbox-buffer--reflinks (node)
  "Return daemon-backed reflink matches for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'reflinks nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--unlinked-references (node)
  "Return daemon-backed unlinked references for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'refs 'unlinked-references nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--time-neighbors (node)
  "Return daemon-backed time neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'time 'time-neighbors nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--task-neighbors (node)
  "Return daemon-backed task neighbors for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'tasks 'task-neighbors nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--bridge-candidates (node)
  "Return daemon-backed bridge candidates for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'bridges 'bridge-candidates nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--dormant-notes (node)
  "Return daemon-backed dormant notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'dormant 'dormant-notes nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--unresolved-tasks (node)
  "Return daemon-backed unresolved tasks for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'unresolved-tasks nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--weakly-integrated-notes (node)
  "Return daemon-backed weakly integrated notes for NODE."
  (org-slipbox-buffer--exploration-section-entries
   node 'unresolved 'weakly-integrated-notes nil (org-slipbox-buffer--current-query-limit)))

(defun org-slipbox-buffer--backlinks (node &optional unique limit)
  "Return backlinks for NODE.
When UNIQUE is non-nil, only return the first occurrence per source
node. LIMIT bounds the number of rows requested."
  (org-slipbox-buffer--exploration-section-entries
   node 'structure 'backlinks unique limit))

(defun org-slipbox-buffer--exploration-section-kind-name (section-kind)
  "Return SECTION-KIND encoded for exploration results."
  (pcase section-kind
    ('backlinks "backlinks")
    ('forward-links "forward-links")
    ('reflinks "reflinks")
    ('unlinked-references "unlinked-references")
    ('time-neighbors "time-neighbors")
    ('task-neighbors "task-neighbors")
    ('bridge-candidates "bridge-candidates")
    ('dormant-notes "dormant-notes")
    ('unresolved-tasks "unresolved-tasks")
    ('weakly-integrated-notes "weakly-integrated-notes")
    (_
     (user-error "Unsupported exploration section kind %S" section-kind))))

(defun org-slipbox-buffer--exploration-cache-key (focus-key lens unique limit)
  "Return the cache key for FOCUS-KEY, exploration LENS, UNIQUE, and LIMIT."
  (list focus-key
        lens
        (or limit org-slipbox-buffer-default-query-limit)
        (and unique t)))

(defun org-slipbox-buffer--exploration-result (node lens &optional unique limit)
  "Return cached exploration results for NODE under LENS."
  (let ((limit (or limit org-slipbox-buffer-default-query-limit)))
    (when-let ((focus-key (or (org-slipbox-buffer--current-focus-key)
                              (plist-get node :node_key))))
    (if-let* ((session org-slipbox-buffer-session)
              (cache-key (org-slipbox-buffer--exploration-cache-key
                          focus-key lens unique limit))
              (cached (assoc cache-key
                             (org-slipbox-buffer-session-lens-cache session))))
        (cdr cached)
      (let ((result (org-slipbox-rpc-explore focus-key lens limit unique)))
        (when-let ((session org-slipbox-buffer-session))
          (let ((cache-key (org-slipbox-buffer--exploration-cache-key
                            focus-key lens unique limit))
                (existing (org-slipbox-buffer-session-lens-cache session)))
            (setf (org-slipbox-buffer-session-lens-cache session)
                  (cons (cons cache-key result)
                        (cl-remove cache-key existing :key #'car :test #'equal)))))
        result)))))

(defun org-slipbox-buffer--exploration-section-entries
    (node lens section-kind &optional unique limit)
  "Return SECTION-KIND entries for NODE under exploration LENS."
  (when-let* ((result (org-slipbox-buffer--exploration-result node lens unique limit))
              (sections (org-slipbox--plist-sequence (plist-get result :sections)))
              (section-name (org-slipbox-buffer--exploration-section-kind-name section-kind))
              (section (seq-find (lambda (candidate)
                                   (equal (plist-get candidate :kind) section-name))
                                 sections)))
    (org-slipbox--plist-sequence (plist-get section :entries))))

(defun org-slipbox-buffer--comparison-cache-key (left-node right-node limit)
  "Return the cache key for LEFT-NODE, RIGHT-NODE, and LIMIT."
  (list (plist-get left-node :node_key)
        (plist-get right-node :node_key)
        (or limit org-slipbox-buffer-default-query-limit)))

(defun org-slipbox-buffer--comparison-result (left-node right-node &optional limit)
  "Return cached comparison results for LEFT-NODE and RIGHT-NODE."
  (let ((limit (or limit (org-slipbox-buffer--current-query-limit))))
    (when-let ((left-key (plist-get left-node :node_key))
               (right-key (plist-get right-node :node_key)))
      (if-let* ((session org-slipbox-buffer-session)
                (cache-key (org-slipbox-buffer--comparison-cache-key
                            left-node right-node limit))
                (cached (assoc cache-key
                               (org-slipbox-buffer-session-comparison-cache session))))
          (cdr cached)
        (let ((result (org-slipbox-rpc-compare-notes left-key right-key limit)))
          (when-let ((session org-slipbox-buffer-session))
            (let ((cache-key (org-slipbox-buffer--comparison-cache-key
                              left-node right-node limit))
                  (existing (org-slipbox-buffer-session-comparison-cache session)))
              (setf (org-slipbox-buffer-session-comparison-cache session)
                    (cons (cons cache-key result)
                          (cl-remove cache-key existing :key #'car :test #'equal)))))
          result)))))

(provide 'org-slipbox-buffer-query)

;;; org-slipbox-buffer-query.el ends here
