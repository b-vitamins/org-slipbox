/*
 * Math rendering via KaTeX, whose output is a self-contained scriptless HTML
 * string and so is set as `innerHTML`. `throwOnError: false` covers TeX KaTeX
 * rejects but not TeX that defeats it: deep nesting exhausts the recursive
 * parser and raises `RangeError`, so the guard rebuilds the same output.
 */

import katex from "katex";
import { type Component } from "solid-js";

import "katex/dist/katex.min.css";

/*
 * The color unrenderable TeX is shown in. KaTeX writes this straight into a
 * `style` attribute, where a custom property resolves against the element.
 */
const ERROR_COLOR = "var(--math-error)";

/**
 * Raw `tex` marked up the way KaTeX marks up TeX it has rejected. Built through
 * the DOM, which escapes the TeX and the reason as it serializes them.
 */
function errorHtml(tex: string, reason: string): string {
  const span = document.createElement("span");
  span.className = "katex-error";
  span.title = reason;
  span.style.color = ERROR_COLOR;
  span.textContent = tex;
  return span.outerHTML;
}

function renderTex(tex: string, displayMode: boolean): string {
  try {
    return katex.renderToString(tex, {
      displayMode,
      throwOnError: false,
      errorColor: ERROR_COLOR,
      output: "htmlAndMathml",
    });
  } catch (error) {
    return errorHtml(tex, error instanceof Error ? error.message : String(error));
  }
}

export const InlineMath: Component<{ tex: string }> = (props) => (
  // eslint-disable-next-line solid/no-innerhtml -- KaTeX output is trusted HTML.
  <span class="org-math org-math--inline" innerHTML={renderTex(props.tex, false)} />
);

export const DisplayMath: Component<{ tex: string }> = (props) => (
  <div class="org-math org-math--display" innerHTML={renderTex(props.tex, true)} />
);
