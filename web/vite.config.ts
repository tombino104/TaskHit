import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { VitePWA } from 'vite-plugin-pwa';

export default defineConfig({
  plugins: [
    svelte(),
    VitePWA({
      registerType: 'autoUpdate',
      manifest: {
        name: 'TaskHit',
        short_name: 'TaskHit',
        description: 'Il calendario condiviso della classe',
        lang: 'it',
        start_url: '/',
        display: 'standalone',
        background_color: '#F1F0EB',
        theme_color: '#F1F0EB',
        icons: [
          { src: 'icon-192.png', sizes: '192x192', type: 'image/png' },
          { src: 'icon-512.png', sizes: '512x512', type: 'image/png' },
          { src: 'icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
        ],
      },
      workbox: {
        // Le API e i temi non vanno mai serviti dalla cache del service worker: ci pensa l'app.
        navigateFallbackDenylist: [/^\/api\//],
        globPatterns: ['**/*.{js,css,html,woff2,png,svg}'],
      },
    }),
  ],
  server: { proxy: { '/api': 'http://127.0.0.1:8080' } },
  // Nessun file incorporato come data: URI: la CSP del server consente solo font serviti da qui.
  build: { target: 'es2022', sourcemap: false, assetsInlineLimit: 0 },
  test: { environment: 'node' },
});
