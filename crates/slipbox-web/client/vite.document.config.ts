/// <reference types="node" />
import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

/*
 * The standalone Org document bundle, built separately from the single-page
 * application's `dist/`.
 *
 * A host serves this directory from any local origin, links `document.css` and
 * imports `document.js`; both names are fixed and unhashed because the host
 * writes them. `base: "./"` keeps the directory relocatable, `assetsInlineLimit:
 * 0` emits KaTeX's fonts as local files beside the stylesheet, and `publicDir`
 * is off so the inventory lists only what this bundle produced.
 */
export default defineConfig({
  base: "./",
  publicDir: false,
  plugins: [solid()],
  build: {
    target: "es2022",
    outDir: "dist-document",
    emptyOutDir: true,
    cssCodeSplit: false,
    assetsInlineLimit: 0,
    // The entry has no dynamic import, so there is nothing to preload and no
    // reason to carry the polyfill that would inject link elements at load.
    modulePreload: false,
    rollupOptions: {
      input: fileURLToPath(new URL("src/document/index.ts", import.meta.url)),
      preserveEntrySignatures: "exports-only",
      output: {
        format: "es",
        entryFileNames: "document.js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: (asset) =>
          asset.names.some((name) => name.endsWith(".css"))
            ? "document.css"
            : "assets/[name]-[hash][extname]",
      },
    },
  },
  preview: {
    port: 4174,
    strictPort: true,
  },
});
