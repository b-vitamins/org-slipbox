/* Client entry point: mount the reading-surface shell into the document. */

import { render } from "solid-js/web";

import { App } from "./App.js";

const root = document.getElementById("root");
if (!root) {
  throw new Error("missing #root mount element");
}

render(() => <App />, root);
