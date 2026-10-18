import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import { ipcBoundaryExemption, ipcBoundaryRules } from './eslint.ipc-boundary.js';

export default tseslint.config(
  { ignores: ['dist', 'src-tauri', 'node_modules', 'coverage'] },
  js.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  ...tseslint.configs.stylisticTypeChecked,
  {
    languageOptions: {
      globals: globals.browser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
    plugins: { 'react-hooks': reactHooks, 'react-refresh': reactRefresh },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['error', { allowConstantExport: true }],
      '@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
    },
  },
  {
    // The host is reached only through src/ipc (swappable transport). See eslint.ipc-boundary.js.
    files: ['src/**/*.{ts,tsx}'],
    rules: ipcBoundaryRules,
  },
  {
    // The transport implementation, and the test that exercises the boundary rules with bypass samples.
    files: ['src/ipc/tauriTransport.ts', 'src/ipc/boundary.lint.test.ts'],
    rules: ipcBoundaryExemption,
  },
  {
    files: ['*.config.{js,ts}', 'eslint.*.js'],
    languageOptions: { globals: globals.node },
    extends: [tseslint.configs.disableTypeChecked],
  },
);
