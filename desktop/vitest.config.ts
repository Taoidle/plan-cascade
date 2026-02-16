import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: [
      'src/**/*.{test,spec}.{ts,tsx}',
      'src/test/integration/**/*.{test,spec}.{ts,tsx}',
    ],
    exclude: [
      'node_modules',
      'dist',
      '.idea',
      '.git',
      '.cache',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html', 'lcov'],
      include: [
        'src/components/ClaudeCodeMode/**/*.{ts,tsx}',
        'src/components/ExpertMode/**/*.{ts,tsx}',
        'src/components/SimpleMode/**/*.{ts,tsx}',
        'src/components/Projects/**/*.{ts,tsx}',
        'src/components/Analytics/**/*.{ts,tsx}',
        'src/components/Settings/**/*.{ts,tsx}',
        'src/components/ModeSwitch.tsx',
        'src/components/SkillMemory/**/*.{ts,tsx}',
        'src/components/shared/**/*.{ts,tsx}',
        'src/store/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
      ],
      exclude: [
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/test/**/*',
      ],
      reportsDirectory: './coverage',
      all: true,
      thresholds: {
        lines: 60,
        functions: 60,
        branches: 60,
        statements: 60,
      },
    },
    testTimeout: 10000,
    hookTimeout: 10000,
  },
});
