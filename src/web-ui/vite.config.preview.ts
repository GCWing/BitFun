import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const webUiRoot = path.resolve(__dirname);

// Vite config for component preview
export default defineConfig({
  root: webUiRoot,
  plugins: [react()],

  // Path resolution
  resolve: {
    alias: {
      "@": path.resolve(webUiRoot, "src"),
      "@/shared": path.resolve(webUiRoot, "src/shared"),
      "@/core": path.resolve(webUiRoot, "src/core"),
      "@/tools": path.resolve(webUiRoot, "src/tools"),
      "@/hooks": path.resolve(webUiRoot, "src/hooks"),
      "@/styles": path.resolve(webUiRoot, "src/component-library/styles"),
      "@/types": path.resolve(webUiRoot, "src/shared/types"),
      "@/utils": path.resolve(webUiRoot, "src/shared/utils"),
      "@components": path.resolve(
        webUiRoot,
        "src/component-library/components"
      ),
    },
  },

  // Component preview server config
  server: {
    port: 3000,
    open: "/preview.html",
    host: true,
  },

  // Build config
  build: {
    outDir: "dist-preview",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        preview: path.resolve(webUiRoot, "preview.html"),
      },
    },
  },
});
