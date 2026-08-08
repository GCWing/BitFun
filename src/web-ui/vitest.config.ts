import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

// Vitest runs in the Node runtime; see src/test/setup.ts for the
// in-memory localStorage polyfill (Node >= 22 exposes a method-less
// webstorage shell that breaks zustand persist and storage helpers).
//
// This config is intentionally separate from vite.config.ts (the production
// Tauri/dev-server config) so the production pipeline stays clean and tests
// can pick the exact transform + alias setup they need.
export default defineConfig({
  plugins: [react()],

  // Path resolution must mirror vite.config.ts so test imports using the
  // `@/...` aliases resolve exactly like production code.
  resolve: {
    dedupe: ['react', 'react-dom'],
    alias: {
      "@": path.resolve(__dirname, "./src"),
      "@/shared": path.resolve(__dirname, "./src/shared"),
      "@/core": path.resolve(__dirname, "./src/core"),
      "@/tools": path.resolve(__dirname, "./src/tools"),
      "@/hooks": path.resolve(__dirname, "./src/hooks"),
      "@/styles": path.resolve(__dirname, "./src/component-library/styles"),
      "@/types": path.resolve(__dirname, "./src/shared/types"),
      "@/utils": path.resolve(__dirname, "./src/shared/utils"),
      "@components": path.resolve(__dirname, "./src/component-library/components"),
    },
  },

  test: {
    setupFiles: ["./src/test/setup.ts"],
  },
});
