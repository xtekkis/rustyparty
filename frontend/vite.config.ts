import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    // The backend runs on its own port in development, so pass its addresses on to it
    proxy: {
      '/ws': { target: 'ws://localhost:3000', ws: true },
      '/rooms': 'http://localhost:3000',
    },
  },
})
