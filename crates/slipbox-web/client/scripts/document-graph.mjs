/*
 * Check the document bundle's boundary against the bundler's own resolved graph:
 * both entries are built in memory and their chunks' module lists compared. The
 * application graph is a control, so the prefixes asserted absent from the
 * document graph must be present in it.
 */

import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "vite";

const CLIENT = resolve(fileURLToPath(new URL("..", import.meta.url)));

/** Application-only source the document bundle must not reach. */
const FORBIDDEN = [
  "src/main.tsx",
  "src/App.tsx",
  "src/api/",
  "src/data/",
  "src/entry/",
  "src/reading/",
];

/** Shared implementation both entries must resolve to the same files. */
const SHARED = [
  "src/org/GrammarLink.tsx",
  "src/org/Math.tsx",
  "src/org/RenderDocument.tsx",
  "src/org/RenderInline.tsx",
  "src/org/SourceBlock.tsx",
  "src/org/assets.tsx",
  "src/org/link-target.ts",
  "src/org/navigation.tsx",
  "src/org/parse-inline.ts",
  "src/org/parse.ts",
  "src/org/tex.ts",
];

/** A rule only `src/org/org.css` declares, so its presence names its source. */
const SHARED_CSS_RULE = ".org-src__chrome";

/**
 * Host capabilities a rendered document must not depend on, searched for by name
 * in the built JS. A name can appear without being called, so a hit is a signal to
 * read the bundle rather than proof of a live dependency.
 */
const FORBIDDEN_GLOBALS = [
  "localStorage",
  "sessionStorage",
  "indexedDB",
  "XMLHttpRequest",
  "EventSource",
  "WebSocket",
  "navigator.sendBeacon",
];

function normalize(id) {
  const bare = id.replace(/^\0/, "").split("?")[0] ?? "";
  const absolute = bare.startsWith(CLIENT) ? bare.slice(CLIENT.length + 1) : bare;
  return absolute.split("\\").join("/");
}

async function graphOf(configFile) {
  const result = await build({
    configFile: join(CLIENT, configFile),
    root: CLIENT,
    logLevel: "warn",
    build: { write: false },
  });
  const bundles = Array.isArray(result) ? result : [result];
  const modules = new Set();
  let css = "";
  let js = "";
  for (const bundle of bundles) {
    for (const item of bundle.output) {
      if (item.type === "chunk") {
        for (const id of Object.keys(item.modules)) {
          modules.add(normalize(id));
        }
        js += item.code;
      } else if (item.fileName.endsWith(".css")) {
        css += String(item.source);
      }
    }
  }
  return { modules, css, js };
}

const matches = (modules, prefix) =>
  [...modules].filter((id) => id === prefix || id.startsWith(prefix));

const documentGraph = await graphOf("vite.document.config.ts");
const application = await graphOf("vite.config.ts");

const failures = [];

for (const prefix of FORBIDDEN) {
  const found = matches(documentGraph.modules, prefix);
  if (found.length > 0) {
    failures.push(`document graph reaches ${prefix}: ${found.join(", ")}`);
  }
  if (matches(application.modules, prefix).length === 0) {
    failures.push(`control failed: the application graph does not reach ${prefix}`);
  }
}

for (const shared of SHARED) {
  if (!documentGraph.modules.has(shared)) {
    failures.push(`document graph is missing shared module ${shared}`);
  }
  if (!application.modules.has(shared)) {
    failures.push(`application graph is missing shared module ${shared}`);
  }
}

for (const [name, graph] of [
  ["document", documentGraph],
  ["application", application],
]) {
  if (!graph.css.includes(SHARED_CSS_RULE)) {
    failures.push(`${name} stylesheet does not carry ${SHARED_CSS_RULE}`);
  }
}

const readingCss = await readFile(join(CLIENT, "src/reading/reading.css"), "utf8");
if (readingCss.includes(SHARED_CSS_RULE)) {
  failures.push(`reading.css still declares ${SHARED_CSS_RULE}`);
}

for (const global of FORBIDDEN_GLOBALS) {
  if (documentGraph.js.includes(global)) {
    failures.push(`document bundle references ${global}`);
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      document: { modules: documentGraph.modules.size },
      application: { modules: application.modules.size },
      shared: SHARED.length,
      forbidden: FORBIDDEN.length,
      failures,
    },
    null,
    2,
  )}\n`,
);

if (failures.length > 0) {
  process.exitCode = 1;
}
