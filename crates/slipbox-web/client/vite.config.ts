/// <reference types="node" />
import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

// Relative asset URLs (`base: "./"`) let the `slipbox` binary embed the whole
// `dist/` tree and serve it from any mount point; no source maps, since they
// would ship inside every binary. `/api` is proxied in development so the
// client reaches the daemon without CORS.
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
    globals: true,
    setupFiles: ["src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
