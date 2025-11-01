import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
    plugins: [react()],
    build: {
        outDir: "./dist/files",
        emptyOutDir: true,
    },
    server: {
        proxy: {
            '/api': {
                target: 'http://localhost:8125',
                changeOrigin: true,
            },
            '/auth': {
                target: 'http://localhost:8125',
                changeOrigin: true,
            },
            '/rpc': {
                target: 'http://localhost:8125',
                changeOrigin: true,
            },
        },
    },
})
