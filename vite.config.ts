import { defineConfig } from "vite";
import { copyFileSync, rmSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async ({ mode }) => {
  const isDemo = mode === "demo";
  return {
    base: "./",
    plugins: [
      react(),
      tailwindcss(),
      {
        // Tauri packages the entire dist/ into the .app bundle, so strip
        // unreferenced assets after build to keep the dmg small.
        // - public/welcome.mp4: nothing in src/ or index.html imports it.
        // - public/logo.svg: nothing references it (last-token.svg is the in-app icon).
        // - public/logo.png: 4MB favicon -> reuse the 192KB app icon.
        // Sources in public/ stay untouched; this only edits dist/.
        name: "slim-dist-for-tauri",
        apply: "build",
        closeBundle() {
          if (isDemo) return;
          const dist = resolve(__dirname, "dist");
          for (const f of ["welcome.mp4", "logo.svg"]) {
            const p = resolve(dist, f);
            if (existsSync(p)) rmSync(p);
          }
          const favicon = resolve(dist, "logo.png");
          const icon = resolve(dist, "last-token.png");
          if (existsSync(favicon) && existsSync(icon)) {
            copyFileSync(icon, favicon);
          }
        },
      },
    ],
    resolve: {
      alias: {
        "@/lib/backend": isDemo
          ? path.resolve(__dirname, "./tests/demo/backend.ts")
          : path.resolve(__dirname, "./src/lib/backend.ts"),
        "@": path.resolve(__dirname, "./src"),
      },
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
