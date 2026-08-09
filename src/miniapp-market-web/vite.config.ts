import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  base: '/miniapp/',
  plugins: [react()],
  server: {
    port: 1431,
    proxy: {
      '/miniapp/api': {
        target: process.env.MARKET_DEV_API || 'http://127.0.0.1:9710',
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
  },
});
