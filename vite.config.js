import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  root: "web",
  build: {
    outDir: resolve("dist"),
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    proxy: {
      "/ws": {
        target: "ws://127.0.0.1:3000",
        ws: true,
      },
    },
  },
  worker: {
    format: "es",
  },
});
