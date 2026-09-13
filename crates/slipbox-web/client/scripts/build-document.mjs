/* Emit the document bundle, content hashes and bundled dependency licences. */

import { createHash } from "node:crypto";
import { copyFile, readFile, readdir, writeFile } from "node:fs/promises";
import { join, posix, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "vite";

const CLIENT = resolve(fileURLToPath(new URL("..", import.meta.url)));
const OUT_DIR = join(CLIENT, "dist-document");
const INVENTORY = "assets.json";
const NOTICES = "THIRD-PARTY-NOTICES.txt";
const HOST = "host.html";
const ENTRY = "document.js";
const STYLESHEET = "document.css";

const LICENCE_NAMES = [
  "LICENSE",
  "LICENSE.md",
  "LICENSE.txt",
  "LICENCE",
  "LICENCE.md",
  "LICENCE.txt",
  "COPYING",
];

function bundledModules(result) {
  const bundles = Array.isArray(result) ? result : [result];
  const ids = new Set();
  for (const bundle of bundles) {
    for (const item of bundle.output) {
      if (item.type === "chunk") {
        for (const id of Object.keys(item.modules)) {
          ids.add(id);
        }
      }
    }
  }
  return ids;
}

function packageOf(id) {
  const marker = id.lastIndexOf("node_modules/");
  if (marker === -1) {
    return null;
  }
  const parts = id.slice(marker + "node_modules/".length).split("/");
  const scoped = parts[0]?.startsWith("@");
  const name = scoped ? parts.slice(0, 2).join("/") : parts[0];
  return name ?? null;
}

async function licenceText(name) {
  for (const candidate of LICENCE_NAMES) {
    try {
      return (await readFile(join(CLIENT, "node_modules", name, candidate), "utf8")).trim();
    } catch {
      continue;
    }
  }
  return null;
}

async function notices(ids) {
  const names = [...new Set([...ids].map(packageOf).filter((name) => name !== null))];
  names.sort();
  const sections = [];
  for (const name of names) {
    const manifest = JSON.parse(
      await readFile(join(CLIENT, "node_modules", name, "package.json"), "utf8"),
    );
    const text = await licenceText(name);
    if (text === null) {
      throw new Error(`${name} ships no licence file; it cannot be redistributed`);
    }
    const rule = "-".repeat(78);
    sections.push(
      `${rule}\n${manifest.name} ${manifest.version} (${manifest.license ?? "see below"})\n${rule}\n\n${text}\n`,
    );
  }
  return `This bundle contains the following third-party software.\n\n${sections.join("\n")}`;
}

async function emitted(directory) {
  const found = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      found.push(...(await emitted(path)));
    } else {
      found.push(path);
    }
  }
  return found;
}

async function inventory() {
  const files = (await emitted(OUT_DIR))
    .map((path) => relative(OUT_DIR, path).split(/[/\\]/).join(posix.sep))
    // The inventory cannot hash itself.
    .filter((path) => path !== INVENTORY)
    .sort();
  const entries = [];
  for (const path of files) {
    const bytes = await readFile(join(OUT_DIR, path));
    entries.push({
      path,
      bytes: bytes.byteLength,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
  for (const required of [ENTRY, STYLESHEET, HOST]) {
    if (!files.includes(required)) {
      throw new Error(`the build produced no ${required}`);
    }
  }
  return { entry: ENTRY, stylesheet: STYLESHEET, host: HOST, files: entries };
}

const result = await build({
  configFile: join(CLIENT, "vite.document.config.ts"),
  root: CLIENT,
  logLevel: "warn",
});

await copyFile(join(CLIENT, "src", "document", HOST), join(OUT_DIR, HOST));
await writeFile(join(OUT_DIR, NOTICES), await notices(bundledModules(result)), "utf8");
await writeFile(
  join(OUT_DIR, INVENTORY),
  `${JSON.stringify(await inventory(), null, 2)}\n`,
  "utf8",
);

const listed = JSON.parse(await readFile(join(OUT_DIR, INVENTORY), "utf8"));
process.stdout.write(
  `dist-document: ${listed.files.length} files, ${listed.files.reduce((total, file) => total + file.bytes, 0)} bytes\n`,
);
