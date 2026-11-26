import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { configDefaults, defineConfig } from "vitest/config";

const host = process.env.TAURI_DEV_HOST;

// Tauri expects a fixed port and must not have Rust errors hidden by Vite's screen clearing.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // Served from disk inside the app, so one larger chunk costs nothing.
  build: { chunkSizeWarningLimit: 1500 },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**", "**/.solace/**"] },
  },
  // Worktrees of parallel tasks hold copies of the tests.
  test: { exclude: [...configDefaults.exclude, "**/.solace/**"] },
});
