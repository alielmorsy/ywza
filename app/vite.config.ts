import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

const preview = "/src/preview/tauri.ts";
// Set by `tauri android dev` so the phone can reach the dev server.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(({ mode }) => ({
  plugins: [tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: mode !== "preview",
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
  },
  build: { target: "es2022" },
  // `yarn preview:ui`: run the UI in a normal browser with fake data instead of Tauri.
  resolve: mode === "preview"
    ? { alias: { "@tauri-apps/api/core": preview, "@tauri-apps/api/event": preview, "@tauri-apps/api/window": preview, "@tauri-apps/plugin-opener": preview } }
    : {},
}));
