import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: '/skin/',
  plugins: [react()],
  server: {
    port: 1432,
    proxy: {
      '/skin/api': {
        target: process.env.SKIN_MARKET_DEV_API || 'http://127.0.0.1:9720',
        changeOrigin: true,
      },
      '/miniapp/api': {
        target: process.env.MINIAPP_MARKET_DEV_API || 'http://127.0.0.1:9710',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: false,
  },
});
