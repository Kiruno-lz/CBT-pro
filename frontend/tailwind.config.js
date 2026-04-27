export default {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        surface: {
          base: '#0f172a',
          raised: '#1e293b',
          elevated: '#283548',
          overlay: '#3d4a5c',
        },
        text: {
          primary: '#e8eaf0',
          secondary: '#a8b0bf',
          muted: '#6b7891',
          inverse: '#0f172a',
        },
        accent: {
          cyan: '#22d3ee',
          cyanDim: '#0891b2',
          green: '#34d399',
          greenDim: '#059669',
          red: '#f87171',
          redDim: '#dc2626',
          amber: '#fbbf24',
        },
        border: {
          subtle: '#1e293b',
          default: '#334155',
          strong: '#475569',
        },
      },
      fontFamily: {
        mono: ['JetBrains Mono', 'ui-monospace', 'SFMono-Regular', 'monospace'],
        sans: ['Inter', 'system-ui', 'sans-serif'],
        display: ['Inter', 'system-ui', 'sans-serif'],
      },
      fontSize: {
        '2xs': ['0.625rem', { lineHeight: '0.75rem' }],
        xs: ['0.6875rem', { lineHeight: '0.875rem' }],
        sm: ['0.75rem', { lineHeight: '1rem' }],
        base: ['0.8125rem', { lineHeight: '1.125rem' }],
        lg: ['0.9375rem', { lineHeight: '1.25rem' }],
        xl: ['1.125rem', { lineHeight: '1.5rem' }],
        '2xl': ['1.375rem', { lineHeight: '1.75rem' }],
      },
      spacing: {
        '4.5': '1.125rem',
        '13': '3.25rem',
        '15': '3.75rem',
        '18': '4.5rem',
      },
      animation: {
        'fade-in': 'fade-in 200ms ease-out',
        'slide-up': 'slide-up 300ms cubic-bezier(0.16, 1, 0.3, 1)',
        'pulse-subtle': 'pulse-subtle 2s ease-in-out infinite',
      },
      keyframes: {
        'fade-in': {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        'slide-up': {
          '0%': { opacity: '0', transform: 'translateY(8px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'pulse-subtle': {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.6' },
        },
      },
      transitionTimingFunction: {
        'out-quart': 'cubic-bezier(0.25, 1, 0.5, 1)',
        'out-expo': 'cubic-bezier(0.16, 1, 0.3, 1)',
        'in-out-smooth': 'cubic-bezier(0.45, 0, 0.55, 1)',
      },
    },
  },
  plugins: [],
}