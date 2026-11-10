import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

const preview = "/src/preview/tauri.ts";

export default defineConfig(({ mode }) => ({
  plugins: [tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: mode !== "preview" },
  build: { target: "es2022" },
  // `yarn preview:ui`: run the UI in a normal browser with fake data instead of Tauri.
  resolve: mode === "preview"
    ? { alias: { "@tauri-apps/api/core": preview, "@tauri-apps/api/event": preview, "@tauri-apps/api/window": preview, "@tauri-apps/plugin-opener": preview } }
    : {},
}));
