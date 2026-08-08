import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { versionInjectionPlugin } from "./vite.config.version-plugin";
import { bitfunCanvasRuntimeBundlePlugin } from "./vite.config.canvas-runtime-plugin";

const host = process.env.TAURI_DEV_HOST;

/**
 * Native fs events do not work reliably on UNC network shares (\\server\...,
 * including \\wsl$ / \\wsl.localhost) or on WSL drvfs mounts (/mnt/<drive>).
 * Users upgrading from the polling-based watcher would silently lose HMR
 * there, so print a one-line hint pointing at the VITE_USE_POLLING escape
 * hatch.
 */
function warnIfNativeWatchUnreliable(): void {
  const cwd = process.cwd();
  const looksLikeNetworkOrWslMount =
    cwd.startsWith("\\\\") || /^\/mnt\/[a-z]\//i.test(cwd);
  if (looksLikeNetworkOrWslMount) {
    console.warn(
      `[bitfun] Project path "${cwd}" looks like a network share or WSL mount; ` +
        "native file watching may miss changes here. " +
        "Set VITE_USE_POLLING=1 to restore polling-based HMR.",
    );
  }
}

// https://vite.dev/config/
export default defineConfig(({ mode, command }) => {
  const isProduction = mode === 'production' || (command === 'build' && mode !== 'development');

  if (command === 'serve' && !process.env.VITE_USE_POLLING) {
    warnIfNativeWatchUnreliable();
  }

  return {
    plugins: [
      react(),
      bitfunCanvasRuntimeBundlePlugin(),
      versionInjectionPlugin()
    ],

    // Path resolution
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

  css: {
    preprocessorOptions: {
      scss: {
        // SCSS preprocessing options (sourcemap is controlled by build.sourcemap)
      },
    },
    // dev mode enabled, release mode disabled
    devSourcemap: !isProduction,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1422,
    // Tauri devUrl is fixed to http://localhost:1422.
    // If Vite silently falls back to another port, the desktop webview stays blank.
    strictPort: true,
    host: host || "localhost",
    hmr: {
      protocol: "ws",
      host: host || "localhost",
      port: 1421,
    },
    // Allow access to workspace root for dependencies like monaco-editor
    fs: {
      allow: [
        path.resolve(__dirname, '../../'), // Workspace root
      ],
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri` and `apps`
      ignored: ["**/src-tauri/**", "**/apps/**"],
      // Native fs events by default (polling burned CPU scanning ~1.7k files
      // every 100ms). Escape hatch for network drives / exotic filesystems:
      // set VITE_USE_POLLING=1 to re-enable polling.
      ...(process.env.VITE_USE_POLLING
        ? { usePolling: true, interval: 1000 }
        : {}),
    },
  },

  // Optimize dependency pre-building
  optimizeDeps: {
    // Exclude dependencies that need to be dynamically loaded
    exclude: [],
    // Force pre-building dependencies
    // Resolve Vite 7 and React 18 compatibility issues
    include: [
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-runtime',
      'react/jsx-dev-runtime',
      'mermaid',
      'mermaid/dist/mermaid.esm.min.mjs',
    ],
  },

  // Build options
  build: {
    // Enable CSS code splitting
    cssCodeSplit: true,
    // release version disable sourcemap, dev/debug version enable
    sourcemap: !isProduction,
    // Output to the project root directory dist/
    outDir: '../../dist',
    // Empty the output directory
    emptyOutDir: true,
  }
  };
});
