import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("node_modules/react/") || id.includes("node_modules/react-dom/") || id.includes("node_modules/scheduler/")) {
            return "react-vendor";
          }
          if (id.includes("node_modules/@radix-ui/")) {
            return "radix-vendor";
          }
          if (id.includes("node_modules/lucide-react/")) {
            return "icons-vendor";
          }
          if (
            id.includes("node_modules/@tauri-apps/") ||
            id.includes("node_modules/zustand/") ||
            id.includes("node_modules/@tanstack/")
          ) {
            return "tauri-vendor";
          }
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/crates/**"],
    },
  },
});
