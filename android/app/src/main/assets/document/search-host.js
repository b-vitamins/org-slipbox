/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

let mount = null;
let handle = null;
let current = null;
let queued = false;

function element() {
  return document.getElementById("results");
}

function report(key) {
  const channel = window.slipboxSearch;
  if (!channel || !current) return;
  channel.postMessage(JSON.stringify({ token: current.token, key }));
}

function paint() {
  const style = element().style;
  for (const name of Object.keys(current.presentation.css)) {
    style.setProperty(name, current.presentation.css[name]);
  }
  document.documentElement.style.setProperty(
    "color-scheme",
    current.presentation.theme,
  );
}

function options() {
  return {
    results: current.results,
    theme: current.presentation.theme,
    onOpen: report,
  };
}

function apply() {
  paint();
  if (handle) {
    handle.update(options());
  } else {
    handle = mount(element(), options());
  }
  document.documentElement.dataset.slipboxState = "ready";
}

function present(payload) {
  current = payload;
  if (mount) apply();
  else queued = true;
}

function dispose() {
  queued = false;
  current = null;
  if (handle) {
    handle.dispose();
    handle = null;
  }
  document.documentElement.dataset.slipboxState = "disposed";
}

window.slipboxSearchHost = { present, dispose };
document.documentElement.dataset.slipboxState = "loading";

import("./document.js").then(
  (bundle) => {
    mount = bundle.mountOrgSearch;
    if (queued) {
      queued = false;
      apply();
    }
  },
  () => {
    document.documentElement.dataset.slipboxState = "failed";
  },
);
