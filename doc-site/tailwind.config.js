/** @type {import('tailwindcss').Config} */
const defaultTheme = require('tailwindcss/defaultTheme')

export default {
  content: ['./src/**/*.{astro,html,js,jsx,md,mdx,svelte,ts,tsx,vue}'],
  theme: {
    extend: {
      fontFamily: {
        // This makes Inter your default sans font
        sans: ['Inter', ...defaultTheme.fontFamily.sans],
      },
      colors: {
        // The primary vibrant purple from the image
        brand: {
          50: '#eef2ff',
          100: '#e0e7ff',
          500: '#6366f1', // Main button color
          600: '#4f46e5', // Hover state
          900: '#312e81',
        },
        // The specific background colors
        surface: {
          DEFAULT: '#FFFFFF', // Pure white sections
          alt: '#FAFAFA',     // Slightly off-white sections
        }
      },
      boxShadow: {
        // This is the "secret sauce" for those soft, modern cards
        'soft': '0 4px 20px -2px rgba(0, 0, 0, 0.05)',
        'soft-lg': '0 10px 30px -5px rgba(0, 0, 0, 0.08)',
      }
    },
  },
  plugins: [],
}
