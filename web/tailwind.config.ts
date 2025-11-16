import type { Config } from 'tailwindcss'

export default {
    content: [
        './index.html',
        './src/**/*.{js,ts,jsx,tsx}',
    ],
    theme: {
        extend: {
            colors: {
                'gray': {
                    50: '#f9fafb',
                    100: '#f3f4f6',
                    200: '#e5e7eb',
                    300: '#d1d5db',
                    400: '#9ca3af',
                    500: '#6b7280',
                    600: '#4b5563',
                    700: '#374151',
                    800: '#1f2937',
                    900: '#111827',
                    950: '#030712',
                },
                'blue': {
                    500: '#3b82f6',
                    600: '#2563eb',
                },
                'indigo': {
                    500: '#6366f1',
                },
            },
            fontFamily: {
                sans: ['system-ui', 'sans-serif'],
            },
            animation: {
                'spin': 'spin 1s linear infinite',
            },
            keyframes: {
                spin: {
                    'from': { transform: 'rotate(0deg)' },
                    'to': { transform: 'rotate(360deg)' },
                },
            },
        },
    },
    plugins: [],
} satisfies Config
