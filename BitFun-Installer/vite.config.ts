import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],

  clearScreen: false,

  server: {
    port: 1520,
    strictPort: true,
    host: host || 'localhost',
    hmr: {
      protocol: 'ws',
      host: host || 'localhost',
      port: 1521,
    },
  },

  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    // Optimize for minimal size
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
  },

  // Pre-bundle React for Vite 7 compatibility
  optimizeDeps: {
    include: [
      'react',
      'react-dom',
      'react-dom/client',
      'react/jsx-runtime',
    ],
  },
});
