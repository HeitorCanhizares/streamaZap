/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Porta fixa: o Tauri (src-tauri/tauri.conf.json) abre esta URL no `tauri dev`.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: { target: 'chrome110', sourcemap: true },
  test: { environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}'] },
})
