/*
 * Copyright (C) 2026 Ayan Das
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

const HOST_ID = "document";

let mount = null;
let handle = null;
let current = null;
let queued = false;
let restoredToken = null;
let previewOrigin = null;
let originSerial = 0;
let positionTimer = null;
let readingBlocks = [];
let readingMarks = new Map();

const READING_BLOCKS =
  "h1,h2,h3,h4,h5,h6,p,pre,blockquote,li,table,figure,hr";

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
  const position = readingPosition();
  const origin = intent.verb === "glance" ? bindOrigin(intent.origin) : null;
  if (intent.verb === "glance" && !origin) {
    return;
  }
  channel.postMessage(
    JSON.stringify({
      token: current.token,
      verb: intent.verb,
      link: link
        ? { id: link.id, target: link.target, reference: link.reference }
        : null,
      gesture: intent.gesture || null,
      progress: link ? position.progress : null,
      position: link ? position : null,
      origin,
    }),
  );
}

function reportPosition() {
  const channel = window.slipboxDocument;
  if (!channel || !current) {
    return;
  }
  channel.postMessage(
    JSON.stringify({
      token: current.token,
      verb: "position",
      position: readingPosition(),
    }),
  );
}

function schedulePosition() {
  if (positionTimer !== null) {
    clearTimeout(positionTimer);
  }
  positionTimer = setTimeout(() => {
    positionTimer = null;
    reportPosition();
  }, 160);
}

function bindOrigin(element) {
  if (!(element instanceof HTMLElement) || !current) {
    return null;
  }
  if (previewOrigin && previewOrigin !== element) {
    delete previewOrigin.dataset.slipboxPreviewOrigin;
  }
  const identifier = `${current.token}:${++originSerial}`;
  element.dataset.slipboxPreviewOrigin = identifier;
  previewOrigin = element;
  return identifier;
}

function restoreFocus(origin) {
  if (
    !current ||
    !previewOrigin ||
    !previewOrigin.isConnected ||
    previewOrigin.dataset.slipboxPreviewOrigin !== origin
  ) {
    return false;
  }
  previewOrigin.focus({ preventScroll: true });
  return document.activeElement === previewOrigin;
}

function readingProgress() {
  const scroller = document.scrollingElement;
  if (!scroller) {
    return 0;
  }
  const extent = scroller.scrollHeight - scroller.clientHeight;
  return extent <= 0 ? 0 : Math.max(0, Math.min(1, scroller.scrollTop / extent));
}

function hashText(value, seed) {
  let hash = seed;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function stampReadingMarks() {
  const occurrences = new Map();
  readingBlocks = Array.from(element().querySelectorAll(READING_BLOCKS));
  readingMarks = new Map();
  for (const block of readingBlocks) {
    const text = (block.textContent || "").replace(/\s+/g, " ").trim();
    const value = `${block.tagName.toLowerCase()}\0${text}`;
    const fingerprint =
      hashText(value, 0x811c9dc5) + hashText(value, 0x9e3779b9);
    const occurrence = occurrences.get(fingerprint) || 0;
    occurrences.set(fingerprint, occurrence + 1);
    block.dataset.slipboxReadingMark = `${fingerprint}:${occurrence}`;
    readingMarks.set(block.dataset.slipboxReadingMark, block);
  }
}

function readingPosition() {
  const scroller = document.scrollingElement;
  const progress = readingProgress();
  if (!scroller) {
    return { mark: "", progress, offset: 0 };
  }
  const viewportTop = scroller.scrollTop;
  const probe = document.elementFromPoint(scroller.clientWidth / 2, 1);
  const visible =
    probe instanceof Element
      ? probe.closest("[data-slipbox-reading-mark]")
      : null;
  let selected =
    visible && element().contains(visible)
      ? {
          block: visible,
          top: visible.getBoundingClientRect().top + viewportTop,
          height: Math.max(visible.getBoundingClientRect().height, 1),
        }
      : null;
  for (const block of selected ? [] : readingBlocks) {
    const bounds = block.getBoundingClientRect();
    const top = bounds.top + viewportTop;
    const bottom = top + Math.max(bounds.height, 1);
    if (bottom > viewportTop + 1) {
      selected = { block, top, height: Math.max(bounds.height, 1) };
      break;
    }
  }
  if (!selected) {
    return { mark: "", progress, offset: 0 };
  }
  return {
    mark: selected.block.dataset.slipboxReadingMark || "",
    progress,
    offset: Math.max(
      0,
      Math.min(1, (viewportTop - selected.top) / selected.height),
    ),
  };
}

function restorePosition() {
  const token = current.token;
  if (restoredToken === token) {
    return;
  }
  restoredToken = token;
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      if (!current || current.token !== token) {
        return;
      }
      const scroller = document.scrollingElement;
      if (scroller) {
        const position = current.initialPosition;
        const marked = readingMarks.get(position.mark);
        if (marked) {
          const bounds = marked.getBoundingClientRect();
          const top = bounds.top + scroller.scrollTop;
          scroller.scrollTop = top + position.offset * Math.max(bounds.height, 1);
        } else {
          const extent = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
          scroller.scrollTop = position.progress * extent;
        }
      }
    });
  });
}

function assetHref(target) {
  if (!current) {
    return null;
  }
  const bytes = new TextEncoder().encode(target);
  if (bytes.length === 0 || bytes.length > 4096) {
    return null;
  }
  let encoded = "";
  for (const byte of bytes) {
    encoded += byte.toString(16).padStart(2, "0");
  }
  return current.assetBase + encoded;
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
    interceptExternal: true,
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
  paintViewportLimits();
}

function paintViewportLimits() {
  const height = window.visualViewport
    ? window.visualViewport.height
    : window.innerHeight;
  if (!Number.isFinite(height) || height <= 0) {
    return;
  }
  element().style.setProperty(
    "--document-image-max-height",
    `${Math.min(height * 0.68, 720)}px`,
  );
}

function apply() {
  paint();
  if (handle) {
    handle.update(options());
  } else {
    handle = mount(element(), options());
  }
  stampReadingMarks();
  restorePosition();
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
  if (positionTimer !== null) {
    clearTimeout(positionTimer);
    positionTimer = null;
  }
  queued = false;
  current = null;
  restoredToken = null;
  readingBlocks = [];
  readingMarks = new Map();
  if (previewOrigin) {
    delete previewOrigin.dataset.slipboxPreviewOrigin;
    previewOrigin = null;
  }
  if (handle) {
    handle.dispose();
    handle = null;
  }
  state("disposed");
}

window.slipboxHost = { present, restoreFocus, dispose };
window.addEventListener("resize", paintViewportLimits);
window.addEventListener("scroll", schedulePosition, { passive: true });
window.addEventListener("scrollend", reportPosition);
window.addEventListener("pagehide", reportPosition);
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "hidden") {
    reportPosition();
  }
});
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
