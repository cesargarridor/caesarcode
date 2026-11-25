import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  // App de escritorio: el bundle se carga desde disco, el límite por defecto (500 kB) no aplica.
  build: { target: 'es2022', chunkSizeWarningLimit: 1500 },
});
