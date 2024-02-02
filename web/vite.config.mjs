// Plugins
import Vue from '@vitejs/plugin-vue'

// Utilities
import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    Vue(),
  ],
  define: { 'process.env': {} },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
    extensions: [
      '.js',
      '.json',
      '.jsx',
      '.mjs',
      '.ts',
      '.tsx',
      '.vue',
    ],
  },
  server: {
    port: 3000,
    proxy: {
      '/remote/connect': {
        target: 'http://192.168.3.104:9000',
        changeOrigin: true,
        ws: true,
      },
      '/remote': {
        target: 'http://192.168.3.104:9000',
        changeOrigin: true,
      },
    },
  },
})
