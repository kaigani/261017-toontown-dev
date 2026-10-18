import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';
import tseslint from 'typescript-eslint';

import comments from '@eslint-community/eslint-plugin-eslint-comments';
import compat from 'eslint-plugin-compat';

import { ipcBoundaryConfig } from './boundary/eslint.js';

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
    // Platform floor (ADR-0005): application code may use only APIs available in the browserslist targets of
    // package.json (Safari 15.0 = macOS 12.0 WKWebView, Chrome 110 = WebView2 floor).
    // Tests run in Node, not in the webview, and are never bundled into the app.
    files: ['src/**/*.{ts,tsx,mts,cts,js,jsx,mjs,cjs}'],
    ignores: ['src/**/*.test.*', 'src/test/**'],
    plugins: { compat },
    rules: { 'compat/compat': 'error' },
  },
  // The IPC boundary: every JS/TS file of the package (boundary/eslint.js).
  ...ipcBoundaryConfig(comments),
  {
    // JavaScript files are not in a TypeScript project: lint them without type information.
    files: ['**/*.{js,jsx,mjs,cjs}'],
    extends: [tseslint.configs.disableTypeChecked],
  },
  {
    // Inline ESLint configuration is disabled (noInlineConfig); file-specific exceptions are made here, reviewed.
    // Tauri rejects invokes with the plain serialized error value; the mock transport reproduces that.
    files: ['src/ipc/mockTransport.ts'],
    rules: { '@typescript-eslint/prefer-promise-reject-errors': 'off' },
  },
  {
    // guard<T>() returns a type predicate for T, certifyWireText<T>() the certified value; T appearing once is the
    // point of these APIs.
    files: ['src/ipc/schema.ts', 'src/ipc/wireJson.ts'],
    rules: { '@typescript-eslint/no-unnecessary-type-parameters': 'off' },
  },
  {
    // The text-transport tests reproduce Tauri, which rejects with the plain error text (a string), like the mock.
    files: ['src/ipc/wire.test.ts'],
    rules: { '@typescript-eslint/prefer-promise-reject-errors': 'off' },
  },
  {
    files: ['*.config.{js,ts}', 'boundary/**'],
    languageOptions: { globals: globals.node },
    extends: [tseslint.configs.disableTypeChecked],
  },
);
