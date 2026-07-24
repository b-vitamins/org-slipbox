/*
 * Render preview prose to DOM through the inline renderer, so math is typeset
 * and emphasis and verbatim keep their meaning. The wrapper is a `span` carrying
 * the caller's class, so the preview takes the type of the chrome holding it.
 * `documentPreview` sets display math inline, so no display block is rendered.
 */

import { type Component } from "solid-js";

import { RenderInline } from "./RenderInline.jsx";
import type { Inline } from "./types.js";

export const RenderPreview: Component<{
  prose: readonly Inline[];
  class: string;
}> = (props) => (
  <span class={props.class}>
    <RenderInline nodes={props.prose} />
  </span>
);
