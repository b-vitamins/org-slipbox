;;; org-slipbox-graph.el --- Optional graph export for org-slipbox -*- lexical-binding: t; -*-

;; Copyright (C) 2026 org-slipbox contributors

;; Author: Ayan Das <bvits@riseup.net>
;; Maintainer: Ayan Das <bvits@riseup.net>
;; Version: 0.9.0
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

;; Optional Graphviz export commands for `org-slipbox'.

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'org-slipbox-node)
(require 'org-slipbox-rpc)

(defcustom org-slipbox-graph-viewer nil
  "Method used to view generated org-slipbox graphs.

It may be one of the following:
  - a string naming the viewer executable,
  - a function accepting the graph file path,
  - nil to fall back to `view-file'."
  :type '(choice
          (string :tag "Path to executable")
          (function :tag "Function to display graph")
          (const :tag "view-file" nil))
  :group 'org-slipbox)

(defcustom org-slipbox-graph-executable "dot"
  "Path to the Graphviz executable used for rendering."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-graph-filetype "svg"
  "File type to generate when rendering graphs."
  :type 'string
  :group 'org-slipbox)

(defcustom org-slipbox-graph-node-url-prefix "org-protocol://roam-node?node="
  "Prefix used for node URLs embedded in generated graphs.

When non-nil, nodes with explicit IDs receive Graphviz `URL' attributes
using this prefix plus the percent-encoded ID. This is useful for SVG
viewers that can hand `org-protocol' links back to Emacs through
`org-slipbox-protocol-mode'. Set this to nil to omit node URLs."
  :type '(choice
          (string :tag "URL prefix")
          (const :tag "No node URLs" nil))
  :group 'org-slipbox)

(defcustom org-slipbox-graph-link-hidden-types nil
  "Indexed link types hidden from generated graphs.

The current graph backend is built from indexed `id:' links only, so
`\"id\"' is the only supported value here. Any other value causes graph
generation to fail clearly."
  :type '(repeat string)
  :group 'org-slipbox)

(defcustom org-slipbox-graph-max-title-length 100
  "Maximum title length used in generated graph labels."
  :type 'integer
  :group 'org-slipbox)

(defcustom org-slipbox-graph-shorten-titles 'truncate
  "How generated graph labels should shorten long titles.

Recognized values are the symbols `truncate', `wrap', and nil."
  :type '(choice
          (const :tag "truncate" truncate)
          (const :tag "wrap" wrap)
          (const :tag "no" nil))
  :group 'org-slipbox)

(defcustom org-slipbox-graph-generation-hook nil
  "Hook run after `org-slipbox' graph generation succeeds.

Each function is called with two arguments: the temporary DOT file and
the rendered graph file."
  :type 'hook
  :group 'org-slipbox)

;;;###autoload
(defun org-slipbox-graph (&optional arg node)
  "Build and display a graph for NODE.
With no prefix ARG, generate the global graph.
With a plain `\\[universal-argument]', generate the connected component
for NODE.
With a numeric prefix ARG, generate the neighborhood around NODE up to
ARG steps away."
  (interactive
   (list current-prefix-arg
         (and current-prefix-arg
              (org-slipbox-node-at-point t))))
  (org-slipbox-graph--build-file
   arg
   node
   (make-temp-file "org-slipbox-graph-" nil
                   (concat "." org-slipbox-graph-filetype))
   #'org-slipbox-graph--open))

;;;###autoload
(defun org-slipbox-graph-write-dot (&optional arg node file)
  "Write graph DOT for NODE to FILE.
ARG follows the same scope rules as `org-slipbox-graph'."
  (interactive
   (list current-prefix-arg
         (and current-prefix-arg
              (org-slipbox-node-at-point t))
         (read-file-name "Write graph DOT to: "
                         nil nil nil "org-slipbox.dot")))
  (let* ((file (expand-file-name (or file "org-slipbox.dot")))
         (dot (org-slipbox-graph--dot arg node)))
    (org-slipbox-graph--ensure-parent-directory file)
    (with-temp-file file
      (insert dot))
    (when (called-interactively-p 'interactive)
      (message "Wrote graph DOT to %s" file))
    file))

;;;###autoload
(defun org-slipbox-graph-write-file (&optional arg node file)
  "Render a graph for NODE into FILE.
ARG follows the same scope rules as `org-slipbox-graph'."
  (interactive
   (list current-prefix-arg
         (and current-prefix-arg
              (org-slipbox-node-at-point t))
         (read-file-name
          "Write rendered graph to: "
          nil nil nil
          (format "org-slipbox.%s" org-slipbox-graph-filetype))))
  (let ((file (org-slipbox-graph--build-file arg node file)))
    (when (called-interactively-p 'interactive)
      (message "Wrote graph to %s" file))
    file))

(defun org-slipbox-graph--dot (arg node)
  "Return graph DOT for ARG and NODE."
  (let* ((params (org-slipbox-graph--params arg node))
         (response (org-slipbox-rpc-graph-dot params)))
    (plist-get response :dot)))

(defun org-slipbox-graph--params (arg node)
  "Build graph RPC params from ARG and NODE."
  (let ((params
         (list :include_orphans (org-slipbox-rpc--bool (null arg))
               :hidden_link_types (or org-slipbox-graph-link-hidden-types [])
               :max_title_length org-slipbox-graph-max-title-length
               :shorten_titles (and org-slipbox-graph-shorten-titles
                                    (symbol-name org-slipbox-graph-shorten-titles))
               :node_url_prefix org-slipbox-graph-node-url-prefix)))
    (cond
     ((null arg) params)
     ((consp arg)
      (append params
              (list :root_node_key (org-slipbox-graph--require-node-key node))))
     ((integerp arg)
      (append params
              (list :root_node_key (org-slipbox-graph--require-node-key node)
                    :max_distance (abs arg))))
     (t
      (user-error "Unsupported graph prefix value: %S" arg)))))

(defun org-slipbox-graph--require-node-key (node)
  "Return NODE's key or signal a user error."
  (or (plist-get node :node_key)
      (user-error "Graph scope requires an indexed node")))

(defun org-slipbox-graph--build-file (arg node file &optional callback)
  "Render ARG and NODE into FILE, then run CALLBACK with the file path.

On successful generation, `org-slipbox-graph-generation-hook' is run
with the temporary DOT file and final graph FILE."
  (let* ((file (expand-file-name (or file
                                     (format "org-slipbox.%s"
                                             org-slipbox-graph-filetype))))
         (dot-file (make-temp-file "org-slipbox-graph-" nil ".dot")))
    (unwind-protect
        (progn
          (org-slipbox-graph-write-dot arg node dot-file)
          (org-slipbox-graph--render-dot-file dot-file file)
          (when callback
            (funcall callback file))
          (run-hook-with-args 'org-slipbox-graph-generation-hook dot-file file)
          file)
      (when (file-exists-p dot-file)
        (delete-file dot-file)))))

(defun org-slipbox-graph--render-dot-file (dot-file output-file)
  "Render DOT-FILE into OUTPUT-FILE using Graphviz."
  (unless (stringp org-slipbox-graph-executable)
    (user-error "`org-slipbox-graph-executable' must be a string"))
  (unless (executable-find org-slipbox-graph-executable)
    (user-error "Cannot find graphviz executable %s" org-slipbox-graph-executable))
  (org-slipbox-graph--ensure-parent-directory output-file)
  (with-temp-buffer
    (let ((exit-code (call-process org-slipbox-graph-executable
                                   nil
                                   (current-buffer)
                                   nil
                                   dot-file
                                   "-T"
                                   org-slipbox-graph-filetype
                                   "-o"
                                   output-file)))
      (unless (and (integerp exit-code) (zerop exit-code))
        (user-error
         "Graphviz failed: %s"
         (string-trim (buffer-string)))))))

(defun org-slipbox-graph--ensure-parent-directory (file)
  "Create FILE's parent directory when needed."
  (when-let ((directory (file-name-directory file)))
    (make-directory directory t)))

(defun org-slipbox-graph--open (file)
  "Open FILE using `org-slipbox-graph-viewer'."
  (pcase org-slipbox-graph-viewer
    ((pred stringp)
     (if (executable-find org-slipbox-graph-viewer)
         (let ((exit-code (call-process org-slipbox-graph-viewer nil 0 nil file)))
           (unless (and (integerp exit-code) (zerop exit-code))
             (user-error "Graph viewer failed: %s" org-slipbox-graph-viewer)))
       (user-error "Executable not found: %s" org-slipbox-graph-viewer)))
    ((pred functionp)
     (funcall org-slipbox-graph-viewer file))
    ('nil
     (view-file file))
    (_
     (signal 'wrong-type-argument
             `((functionp stringp null) ,org-slipbox-graph-viewer)))))

(provide 'org-slipbox-graph)

;;; org-slipbox-graph.el ends here
