/// <reference types="node" />
import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

// Relative URLs keep embedded assets relocatable.
export default defineConfig({
  base: "./",
  plugins: [solid()],
  build: {
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
  server: {
    proxy: {
      "/api": {
        target: process.env.SLIPBOX_WEB_ORIGIN ?? "http://127.0.0.1:8080",
        changeOrigin: true,
      },
    },
  },
  test: {
    environment: "jsdom",
    // Let jsdom supply storage rather than Node's file-backed implementation.
    execArgv:
      Number.parseInt(process.versions.node, 10) >= 25 ? ["--no-webstorage"] : [],
    globals: true,
    setupFiles: ["src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
