import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const packageDirectory = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [react()],
  build: {
    cssCodeSplit: false,
    emptyOutDir: false,
    lib: {
      cssFileName: "mobile",
      entry: path.resolve(packageDirectory, "src/mobile.ts"),
      fileName: () => "mobile.js",
      formats: ["es"],
    },
    rollupOptions: {
      external: [
        "react",
        "react-dom",
        "react/jsx-runtime",
      ],
    },
    sourcemap: true,
  },
});
