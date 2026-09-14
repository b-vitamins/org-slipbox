/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

const HOST_ID = "document";

let mount = null;
let handle = null;
let current = null;
let queued = false;

function state(value) {
  document.documentElement.dataset.slipboxState = value;
}

function element() {
  return document.getElementById(HOST_ID);
}

function report(intent) {
  const channel = window.slipboxDocument;
  if (!channel || !current) {
    return;
  }
  const link = intent.link;
  channel.postMessage(
    JSON.stringify({
      token: current.token,
      verb: intent.verb,
      link: link
        ? { id: link.id, target: link.target, reference: link.reference }
        : null,
      gesture: intent.gesture || null,
    }),
  );
}

function assetHref(target) {
  return current ? current.assetBase + encodeURIComponent(target) : null;
}

function options() {
  return {
    content: {
      source: current.source.org,
      baseLevel: current.source.baseLevel,
    },
    theme: current.presentation.theme,
    onIntent: report,
    href: () => null,
    resolveAsset: assetHref,
  };
}

function paint() {
  const css = current.presentation.css;
  const style = element().style;
  for (const name of Object.keys(css)) {
    style.setProperty(name, css[name]);
  }
  document.documentElement.style.setProperty(
    "color-scheme",
    current.presentation.theme,
  );
}

function apply() {
  paint();
  if (handle) {
    handle.update(options());
  } else {
    handle = mount(element(), options());
  }
  state("ready");
}

function present(payload) {
  current = payload;
  if (mount) {
    apply();
  } else {
    queued = true;
  }
}

function dispose() {
  queued = false;
  current = null;
  if (handle) {
    handle.dispose();
    handle = null;
  }
  state("disposed");
}

window.slipboxHost = { present, dispose };
state("loading");

import("./document.js").then(
  (bundle) => {
    mount = bundle.mountOrgDocument;
    if (queued) {
      queued = false;
      apply();
    }
  },
  () => {
    state("failed");
  },
);
