/* A small, explicit syntax grammar set rendered as nodes rather than HTML. */

import bash from "highlight.js/lib/languages/bash";
import c from "highlight.js/lib/languages/c";
import cpp from "highlight.js/lib/languages/cpp";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import haskell from "highlight.js/lib/languages/haskell";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import kotlin from "highlight.js/lib/languages/kotlin";
import lisp from "highlight.js/lib/languages/lisp";
import markdown from "highlight.js/lib/languages/markdown";
import plaintext from "highlight.js/lib/languages/plaintext";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import scheme from "highlight.js/lib/languages/scheme";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import highlighter from "highlight.js/lib/core";
import { For, type Component, type JSX } from "solid-js";

type SyntaxNode =
  | { type: "text"; value: string }
  | {
      type: "element";
      properties?: { className?: Array<string> | string };
      children: Array<SyntaxNode>;
    };

const languages = {
  bash,
  c,
  cpp,
  css,
  diff,
  haskell,
  java,
  javascript,
  json,
  kotlin,
  lisp,
  markdown,
  plaintext,
  python,
  rust,
  scheme,
  sql,
  typescript,
  xml,
  yaml,
};

for (const [name, language] of Object.entries(languages)) {
  highlighter.registerLanguage(name, language);
}

highlighter.registerAliases(["sh", "shell-script", "zsh"], {
  languageName: "bash",
});
highlighter.registerAliases(["js", "jsx"], { languageName: "javascript" });
highlighter.registerAliases(["elisp", "emacs-lisp"], { languageName: "lisp" });
highlighter.registerAliases(["fundamental", "text", "txt"], {
  languageName: "plaintext",
});
highlighter.registerAliases("scm", { languageName: "scheme" });
highlighter.registerAliases(["ts", "tsx"], { languageName: "typescript" });
highlighter.registerAliases("html", { languageName: "xml" });
highlighter.registerAliases("yml", { languageName: "yaml" });

function childrenOf(node: Node): Array<SyntaxNode> {
  return Array.from(node.childNodes).flatMap(safeNode);
}

/** Admit only highlighter spans; source-looking markup remains literal text. */
function safeNode(node: Node): Array<SyntaxNode> {
  if (node.nodeType === Node.TEXT_NODE) {
    return [{ type: "text", value: node.textContent ?? "" }];
  }
  if (!(node instanceof HTMLElement)) {
    return [];
  }
  const children = childrenOf(node);
  if (node.tagName !== "SPAN") {
    return children;
  }
  const className = Array.from(node.classList).filter((name) =>
    /^hljs-[a-z0-9_-]+$/.test(name),
  );
  return className.length === 0
    ? children
    : [{ type: "element", properties: { className }, children }];
}

function tokens(language: string | null, code: string): Array<SyntaxNode> {
  const normalized = language?.trim().toLowerCase() ?? "";
  if (normalized.length === 0 || !highlighter.getLanguage(normalized)) {
    return [{ type: "text", value: code }];
  }
  const highlighted = highlighter.highlight(code, { language: normalized }).value;
  const document = new DOMParser().parseFromString(highlighted, "text/html");
  return childrenOf(document.body);
}

function linesOf(nodes: Array<SyntaxNode>): Array<Array<SyntaxNode>> {
  const lines: Array<Array<SyntaxNode>> = [[]];
  for (const node of nodes) {
    const fragments = linesOfNode(node);
    lines[lines.length - 1]?.push(...(fragments[0] ?? []));
    for (const fragment of fragments.slice(1)) {
      lines.push(fragment);
    }
  }
  return lines;
}

function linesOfNode(node: SyntaxNode): Array<Array<SyntaxNode>> {
  if (node.type === "text") {
    return node.value.split("\n").map((value) =>
      value.length === 0 ? [] : [{ type: "text", value }],
    );
  }
  return linesOf(node.children).map((children) =>
    children.length === 0 ? [] : [{ ...node, children }],
  );
}

function leadingColumns(line: string): number {
  let columns = 0;
  for (const character of line) {
    if (character === " ") {
      columns += 1;
    } else if (character === "\t") {
      columns += 4 - (columns % 4);
    } else {
      break;
    }
  }
  return columns;
}

const SyntaxToken: Component<{ node: SyntaxNode }> = (props): JSX.Element => {
  if (props.node.type === "text") {
    return props.node.value;
  }
  const declared = props.node.properties?.className;
  const className = Array.isArray(declared) ? declared.join(" ") : declared;
  return (
    <span class={className}>
      <For each={props.node.children}>{(node) => <SyntaxToken node={node} />}</For>
    </span>
  );
};

export const SyntaxCode: Component<{
  language: string | null;
  code: string;
  ref: (element: HTMLElement) => void;
}> = (props) => {
  const renderedLines = linesOf(tokens(props.language, props.code));
  const sourceLines = props.code.split("\n");
  return (
    <code class="hljs" ref={props.ref}>
      <For each={renderedLines}>
        {(line, index) => (
          <>
            <span
              class="org-src__line"
              style={{
                "--source-indent": Math.min(
                  leadingColumns(sourceLines[index()] ?? ""),
                  12,
                ),
              }}
            >
              <For each={line}>{(node) => <SyntaxToken node={node} />}</For>
            </span>
            {index() < renderedLines.length - 1 ? "\n" : null}
          </>
        )}
      </For>
    </code>
  );
};
