import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [
    sveltekit({
      // A single-page app: every address the server does not know gets index.html (D16).
      adapter: adapter({ fallback: 'index.html', precompress: true }),
      // The server checks for a new build itself; no polling of version.json.
      version: { pollInterval: 0 },
    }),
  ],
  // `bun run dev` talks to a local `amigo-sofarr serve` on its default port.
  server: { proxy: { '/api': 'http://127.0.0.1:7676' } },
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
