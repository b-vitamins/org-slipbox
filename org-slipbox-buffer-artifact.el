;;; org-slipbox-buffer-artifact.el --- Artifact helpers for org-slipbox context buffers -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Ayan Das

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.16.0
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

;; Artifact helpers for org-slipbox context buffers.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'subr-x)
(require 'org-slipbox-buffer-state)
(require 'org-slipbox-rpc)

(defun org-slipbox-buffer--artifact-save-scope-choices (session)
  "Return saveable artifact scope choices for dedicated SESSION."
  (let ((choices
         (list
          (cons (if (org-slipbox-buffer--comparison-active-p session)
                    "Current comparison"
                  "Current lens view")
                'current))))
    (when (org-slipbox-buffer--trail-active-p session)
      (setq choices
            (append choices
                    '(("Current trail" . trail)
                      ("Current trail slice" . trail-slice)))))
    choices))

(defun org-slipbox-buffer--read-artifact-save-scope (session)
  "Read an artifact save scope for dedicated SESSION."
  (let ((choices (org-slipbox-buffer--artifact-save-scope-choices session)))
    (if (= (length choices) 1)
        (cdar choices)
      (cdr (assoc (completing-read "Save artifact from: "
                                   choices
                                   nil
                                   t
                                   nil
                                   nil
                                   (caar choices))
                  choices)))))

(defun org-slipbox-buffer--artifact-default-title (session scope)
  "Return a default durable artifact title for SESSION and save SCOPE."
  (let* ((snapshot (org-slipbox-buffer--history-snapshot session))
         (node (plist-get snapshot :current-node))
         (compare-target (plist-get snapshot :compare-target))
         (trail (org-slipbox-buffer--trail session))
         (trail-start (and trail (plist-get (car trail) :current-node)))
         (trail-index (or (org-slipbox-buffer--trail-position session) 0))
         (detached (org-slipbox-buffer--trail-detached-p session)))
    (pcase scope
      ('current
       (if compare-target
           (format "%s vs %s"
                   (plist-get node :title)
                   (plist-get compare-target :title))
         (format "%s (%s)"
                 (plist-get node :title)
                 (symbol-name (plist-get snapshot :active-lens)))))
      ('trail
       (format "Trail from %s (%s steps%s)"
               (plist-get trail-start :title)
               (length trail)
               (if detached " + branch" "")))
      ('trail-slice
       (format "Trail slice from %s (step %s of %s%s)"
               (plist-get trail-start :title)
               (1+ trail-index)
               (length trail)
               (if detached " + branch" "")))
      (_
       (user-error "Unsupported artifact save scope %S" scope)))))

(defun org-slipbox-buffer--artifact-default-id (title)
  "Return a stable default artifact identifier for TITLE."
  (let* ((slug (downcase (string-trim (or title ""))))
         (slug (replace-regexp-in-string "[^[:alnum:]]+" "-" slug))
         (slug (string-trim slug "-+" "-+")))
    (if (string-empty-p slug)
        "artifact"
      slug)))

(defun org-slipbox-buffer--read-artifact-title (session scope)
  "Prompt for a durable artifact title for SESSION and save SCOPE."
  (let* ((default (org-slipbox-buffer--artifact-default-title session scope))
         (value (read-string (format "Artifact title (%s): " default)
                             nil
                             nil
                             default)))
    (if (string-empty-p (string-trim value))
        default
      (string-trim value))))

(defun org-slipbox-buffer--read-artifact-id (title)
  "Prompt for a durable artifact identifier using TITLE as context."
  (let* ((default (org-slipbox-buffer--artifact-default-id title))
         (value (read-string (format "Artifact id (%s): " default)
                             nil
                             nil
                             default)))
    (if (string-empty-p (string-trim value))
        default
      (string-trim value))))

(defun org-slipbox-buffer--artifact-summaries ()
  "Return saved exploration artifact summaries through the daemon."
  (org-slipbox--plist-sequence
   (plist-get (org-slipbox-rpc-list-exploration-artifacts) :artifacts)))

(defun org-slipbox-buffer--artifact-summary-choice (summary)
  "Return a completing-read choice for saved artifact SUMMARY."
  (cons (format "%s [%s] <%s>"
                (plist-get summary :title)
                (plist-get summary :kind)
                (plist-get summary :artifact_id))
        summary))

(defun org-slipbox-buffer--read-artifact-summary ()
  "Read a saved exploration artifact summary."
  (let* ((summaries (org-slipbox-buffer--artifact-summaries))
         (choices (mapcar #'org-slipbox-buffer--artifact-summary-choice summaries)))
    (unless choices
      (user-error "No saved exploration artifacts"))
    (cdr (assoc (completing-read "Open artifact: " choices nil t nil nil (caar choices))
                choices))))

(defun org-slipbox-buffer--confirm-artifact-overwrite (artifact-id)
  "Prompt before overwriting saved ARTIFACT-ID."
  (when-let ((existing
              (seq-find (lambda (summary)
                          (equal (plist-get summary :artifact_id) artifact-id))
                        (org-slipbox-buffer--artifact-summaries))))
    (unless (y-or-n-p
             (format "Overwrite exploration artifact %s (%s)? "
                     artifact-id
                     (plist-get existing :title)))
      (user-error "Aborted artifact save"))))

(defun org-slipbox-buffer--required-node-key (node context)
  "Return NODE's required node key for CONTEXT, or signal a user error."
  (or (plist-get node :node_key)
      (user-error "Current %s does not have a node key" context)))

(defun org-slipbox-buffer--required-focus-key (snapshot key context)
  "Return SNAPSHOT's required focus KEY for CONTEXT, or signal a user error."
  (or (plist-get snapshot key)
      (org-slipbox-buffer--required-node-key
       (plist-get snapshot
                  (pcase key
                    (:root-focus-key :root-node)
                    (:current-focus-key :current-node)
                    (_ (user-error "Unsupported focus slot %S" key))))
       context)))

(defun org-slipbox-buffer--artifact-lens-symbol (value)
  "Return VALUE normalized as an exploration lens symbol."
  (if (symbolp value) value (intern value)))

(defun org-slipbox-buffer--artifact-comparison-group-symbol (value)
  "Return VALUE normalized as a comparison-group symbol."
  (if (symbolp value) value (intern (or value "all"))))

(defun org-slipbox-buffer--section-args (section)
  "Return SECTION argument plist, or nil for a bare function SECTION."
  (pcase section
    ((pred functionp) nil)
    (`(,_ . ,args) args)
    (_
     (user-error "Invalid org-slipbox buffer section specification: %S" section))))

(defun org-slipbox-buffer--structure-section-query-shape (section)
  "Return representable structure-query shape metadata for SECTION.
Signal a user error when SECTION changes the structure view in a way the
session and saved artifact models cannot encode faithfully."
  (let* ((function (org-slipbox-buffer--section-function section))
         (args (org-slipbox-buffer--section-args section))
         (allowed-filter-key
          (pcase function
            ('org-slipbox-buffer-backlinks-section :show-backlink-p)
            ('org-slipbox-buffer-forward-links-section :show-forward-link-p)
            (_
             (user-error "Unsupported structure section %S" function))))
         (explicit-unique-p nil)
         (unique nil)
         (explicit-limit-p nil)
         (limit org-slipbox-buffer-default-query-limit))
    (while args
      (let ((key (pop args))
            (value (pop args)))
        (pcase key
          (:unique
           (setq explicit-unique-p t)
           (setq unique (and value t)))
          (:limit
           (setq explicit-limit-p t)
           (setq limit value))
          (:section-heading nil)
          ((guard (eq key allowed-filter-key))
           (when value
             (user-error
              "Current structure lens plan cannot be saved faithfully: %S uses %S"
              function
              key)))
          (_
           (user-error
            "Current structure lens plan cannot be saved faithfully: %S uses unsupported option %S"
            function
            key)))))
    `(:function ,function
      :explicit-unique-p ,explicit-unique-p
      :unique ,unique
      :explicit-limit-p ,explicit-limit-p
      :limit ,limit)))

(defun org-slipbox-buffer--structure-plan-query-options (fallback-limit fallback-unique)
  "Return effective structure query options for the active dedicated plan.
FALLBACK-LIMIT and FALLBACK-UNIQUE supply the session-owned values when the
dedicated plan omits explicit structure-query modifiers."
  (let ((plan (org-slipbox-buffer--dedicated-section-plan 'structure))
        backlinks-shape
        forward-links-shape)
    (dolist (section plan)
      (pcase (org-slipbox-buffer--section-function section)
        ((or 'org-slipbox-buffer-node-section
             'org-slipbox-buffer-refs-section)
         nil)
        ('org-slipbox-buffer-backlinks-section
         (when backlinks-shape
           (user-error
            "Current structure lens plan cannot be saved faithfully: multiple backlink sections are not representable"))
         (setq backlinks-shape
               (org-slipbox-buffer--structure-section-query-shape section)))
        ('org-slipbox-buffer-forward-links-section
         (when forward-links-shape
           (user-error
            "Current structure lens plan cannot be saved faithfully: multiple forward-link sections are not representable"))
         (setq forward-links-shape
               (org-slipbox-buffer--structure-section-query-shape section)))
        (_
         (user-error
          "Current structure lens plan cannot be saved faithfully: section %S is not part of the saved structure lens model"
          (org-slipbox-buffer--section-function section)))))
    (unless (and backlinks-shape forward-links-shape)
      (user-error
       "Current structure lens plan cannot be saved faithfully: it must include exactly one backlinks section and one forward-links section"))
    (let* ((backlinks-options
            `(:limit ,(if (plist-get backlinks-shape :explicit-limit-p)
                          (plist-get backlinks-shape :limit)
                        fallback-limit)
              :unique ,(if (plist-get backlinks-shape :explicit-unique-p)
                           (plist-get backlinks-shape :unique)
                         fallback-unique)))
           (forward-links-options
            `(:limit ,(if (plist-get forward-links-shape :explicit-limit-p)
                          (plist-get forward-links-shape :limit)
                        fallback-limit)
              :unique ,(if (plist-get forward-links-shape :explicit-unique-p)
                           (plist-get forward-links-shape :unique)
                         fallback-unique))))
      (unless (equal backlinks-options forward-links-options)
        (user-error
         "Current structure lens plan cannot be saved faithfully: backlinks and forward links must use the same effective :unique and :limit"))
      backlinks-options)))

(defun org-slipbox-buffer--saved-lens-query-options (snapshot)
  "Return representable saved-query options for SNAPSHOT."
  (if (eq (plist-get snapshot :active-lens) 'structure)
      (org-slipbox-buffer--structure-plan-query-options
       (or (plist-get snapshot :query-limit)
           org-slipbox-buffer-default-query-limit)
       (and (plist-get snapshot :structure-unique) t))
    `(:limit ,(or (plist-get snapshot :query-limit)
                  org-slipbox-buffer-default-query-limit)
      :unique nil)))

(defun org-slipbox-buffer--saved-lens-view-artifact (snapshot)
  "Return a saved lens-view artifact plist from dedicated SNAPSHOT."
  (let ((lens (plist-get snapshot :active-lens))
        (query-options (org-slipbox-buffer--saved-lens-query-options snapshot)))
    `(:kind "lens-view"
      :root_node_key
      ,(org-slipbox-buffer--required-focus-key
        snapshot :root-focus-key "root focus")
      :current_node_key
      ,(org-slipbox-buffer--required-focus-key
        snapshot :current-focus-key "node focus")
      :lens ,(symbol-name lens)
      :limit ,(plist-get query-options :limit)
      :unique ,(org-slipbox-rpc--bool (plist-get query-options :unique))
      :frozen_context
      ,(org-slipbox-rpc--bool (plist-get snapshot :frozen-context)))))

(defun org-slipbox-buffer--validate-restored-snapshot (snapshot)
  "Validate that restored dedicated SNAPSHOT can replay faithfully here."
  (when (eq (plist-get snapshot :active-lens) 'structure)
    (let* ((requested-limit (or (plist-get snapshot :query-limit)
                                org-slipbox-buffer-default-query-limit))
           (requested-unique (and (plist-get snapshot :structure-unique) t))
           (effective
            (org-slipbox-buffer--structure-plan-query-options
             requested-limit
             requested-unique)))
      (unless (equal effective
                     `(:limit ,requested-limit :unique ,requested-unique))
        (user-error
         "Current structure lens plan cannot replay this artifact faithfully: saved and effective structure query semantics differ"))))
  snapshot)

(defun org-slipbox-buffer--snapshot-from-executed-lens-view (execution)
  "Return a dedicated snapshot restored from executed lens-view EXECUTION."
  (let ((artifact (plist-get execution :artifact)))
    (org-slipbox-buffer--validate-restored-snapshot
     `(:current-node ,(plist-get execution :current_note)
       :root-node ,(plist-get execution :root_note)
       :current-focus-key ,(plist-get artifact :current_node_key)
       :root-focus-key ,(plist-get artifact :root_node_key)
       :active-lens ,(org-slipbox-buffer--artifact-lens-symbol
                      (plist-get artifact :lens))
       :compare-target nil
       :comparison-group all
       :query-limit ,(plist-get artifact :limit)
       :structure-unique ,(and (plist-get artifact :unique) t)
       :frozen-context ,(plist-get artifact :frozen_context)))))

(defun org-slipbox-buffer--snapshot-from-executed-comparison (execution)
  "Return a dedicated snapshot restored from executed comparison EXECUTION."
  (let ((artifact (plist-get execution :artifact))
        (result (plist-get execution :result)))
    (org-slipbox-buffer--validate-restored-snapshot
     `(:current-node ,(plist-get result :left_note)
       :root-node ,(plist-get execution :root_note)
       :current-focus-key ,(plist-get (plist-get result :left_note) :node_key)
       :root-focus-key ,(plist-get (plist-get execution :root_note) :node_key)
       :active-lens ,(org-slipbox-buffer--artifact-lens-symbol
                      (plist-get artifact :active_lens))
       :compare-target ,(plist-get result :right_note)
       :comparison-group
       ,(org-slipbox-buffer--artifact-comparison-group-symbol
         (plist-get artifact :comparison_group))
       :query-limit ,(plist-get artifact :limit)
       :structure-unique ,(and (plist-get artifact :structure_unique) t)
       :frozen-context ,(plist-get artifact :frozen_context)))))

(defun org-slipbox-buffer--snapshot-from-executed-trail-step (step)
  "Return a dedicated snapshot restored from executed trail STEP."
  (pcase (plist-get step :kind)
    ("lens-view"
     (org-slipbox-buffer--snapshot-from-executed-lens-view step))
    ("comparison"
     (org-slipbox-buffer--snapshot-from-executed-comparison step))
    (_
     (user-error "Unsupported executed trail step kind %S"
                 (plist-get step :kind)))))

(defun org-slipbox-buffer--restore-trail-state (session replay)
  "Restore dedicated SESSION from executed trail REPLAY."
  (let* ((steps (mapcar #'org-slipbox-buffer--snapshot-from-executed-trail-step
                        (org-slipbox--plist-sequence (plist-get replay :steps))))
         (cursor (plist-get replay :cursor))
         (detached-step
          (when-let ((step (plist-get replay :detached_step)))
            (org-slipbox-buffer--snapshot-from-executed-trail-step step)))
         (current-snapshot (or detached-step (nth cursor steps))))
    (unless current-snapshot
      (user-error "Executed trail artifact did not yield a current cockpit state"))
    (setf (org-slipbox-buffer-session-history session) nil
          (org-slipbox-buffer-session-future session) nil
          (org-slipbox-buffer-session-trail session) steps
          (org-slipbox-buffer-session-trail-index session) cursor)
    (org-slipbox-buffer--apply-history-snapshot session current-snapshot)
    session))

(defun org-slipbox-buffer--restore-executed-artifact (session executed)
  "Restore dedicated SESSION from executed exploration artifact EXECUTED."
  (pcase (plist-get executed :kind)
    ("lens-view"
     (setf (org-slipbox-buffer-session-history session) nil
           (org-slipbox-buffer-session-future session) nil
           (org-slipbox-buffer-session-trail session) nil
           (org-slipbox-buffer-session-trail-index session) nil)
     (org-slipbox-buffer--apply-history-snapshot
      session
      (org-slipbox-buffer--snapshot-from-executed-lens-view executed)))
    ("comparison"
     (setf (org-slipbox-buffer-session-history session) nil
           (org-slipbox-buffer-session-future session) nil
           (org-slipbox-buffer-session-trail session) nil
           (org-slipbox-buffer-session-trail-index session) nil)
     (org-slipbox-buffer--apply-history-snapshot
      session
      (org-slipbox-buffer--snapshot-from-executed-comparison executed)))
    ("trail"
     (org-slipbox-buffer--restore-trail-state
      session
      (plist-get executed :replay)))
    (_
     (user-error "Unsupported executed artifact kind %S"
                 (plist-get executed :kind))))
  session)

(defun org-slipbox-buffer--saved-comparison-artifact (snapshot)
  "Return a saved comparison artifact plist from dedicated SNAPSHOT."
  (let ((root-node (plist-get snapshot :root-node))
        (left-node (plist-get snapshot :current-node))
        (right-node (plist-get snapshot :compare-target)))
    `(:kind "comparison"
      :root_node_key ,(org-slipbox-buffer--required-node-key root-node "root node")
      :left_node_key ,(org-slipbox-buffer--required-node-key left-node "comparison source")
      :right_node_key ,(org-slipbox-buffer--required-node-key right-node "comparison target")
      :active_lens ,(symbol-name (plist-get snapshot :active-lens))
      :structure_unique
      ,(org-slipbox-rpc--bool (plist-get snapshot :structure-unique))
      :comparison_group ,(symbol-name (plist-get snapshot :comparison-group))
      :limit ,(or (plist-get snapshot :query-limit)
                  org-slipbox-buffer-default-query-limit)
      :frozen_context
      ,(org-slipbox-rpc--bool (plist-get snapshot :frozen-context)))))

(defun org-slipbox-buffer--saved-trail-step (snapshot)
  "Return a saved trail step plist from dedicated SNAPSHOT."
  (if (plist-get snapshot :compare-target)
      (org-slipbox-buffer--saved-comparison-artifact snapshot)
    (org-slipbox-buffer--saved-lens-view-artifact snapshot)))

(defun org-slipbox-buffer--saved-trail-artifact (session scope)
  "Return a saved trail artifact plist for SESSION and save SCOPE."
  (let* ((trail (copy-tree (org-slipbox-buffer--trail session)))
         (trail-index (org-slipbox-buffer--trail-position session)))
    (unless trail
      (user-error "No active trail to save"))
    (when (null trail-index)
      (user-error "Current trail does not have an active cursor"))
    (let* ((steps-snapshots (pcase scope
                              ('trail trail)
                              ('trail-slice (cl-subseq trail 0 (1+ trail-index)))
                              (_ (user-error "Unsupported trail save scope %S" scope))))
           (detached-step (and (org-slipbox-buffer--trail-detached-p session)
                               (org-slipbox-buffer--saved-trail-step
                                (org-slipbox-buffer--history-snapshot session)))))
      `(:kind "trail"
        :steps ,(mapcar #'org-slipbox-buffer--saved-trail-step steps-snapshots)
        :cursor ,(if (eq scope 'trail)
                     trail-index
                   (1- (length steps-snapshots)))
        :detached_step ,detached-step))))

(defun org-slipbox-buffer--saved-artifact-payload (session scope)
  "Return a saved artifact payload plist for dedicated SESSION and save SCOPE."
  (let ((snapshot (org-slipbox-buffer--history-snapshot session)))
    (pcase scope
      ('current
       (if (plist-get snapshot :compare-target)
           (org-slipbox-buffer--saved-comparison-artifact snapshot)
         (org-slipbox-buffer--saved-lens-view-artifact snapshot)))
      ((or 'trail 'trail-slice)
       (org-slipbox-buffer--saved-trail-artifact session scope))
      (_
       (user-error "Unsupported artifact save scope %S" scope)))))

(defun org-slipbox-buffer--saved-artifact (session scope artifact-id title)
  "Return a durable saved artifact plist.
SESSION supplies the current cockpit state, SCOPE selects what to save,
and ARTIFACT-ID with TITLE define durable metadata."
  (append `(:artifact_id ,artifact-id
            :title ,title)
          (org-slipbox-buffer--saved-artifact-payload session scope)))

(provide 'org-slipbox-buffer-artifact)

;;; org-slipbox-buffer-artifact.el ends here
