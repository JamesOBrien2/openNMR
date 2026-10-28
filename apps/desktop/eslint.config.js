import js from '@eslint/js';
import { defineConfig } from 'eslint/config';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

export default defineConfig(
  { ignores: ['dist/', 'src-tauri/', 'public/preview-data/'] },
  js.configs.recommended,
  ts.configs.strict,
  svelte.configs.recommended,
  svelte.configs.prettier,
  { languageOptions: { globals: globals.browser } },
  // TypeScript checks undefined names itself; no-undef misreports type-only globals.
  { files: ['**/*.ts', '**/*.svelte'], rules: { 'no-undef': 'off' } },
  // A component's setup runs top to bottom, so reading a `const` declared further
  // down throws while it mounts and the component never appears. Function
  // declarations hoist, so they stay allowed.
  {
    files: ['**/*.ts', '**/*.svelte'],
    rules: {
      'no-use-before-define': 'off',
      '@typescript-eslint/no-use-before-define': [
        'error',
        { functions: false, variables: true, classes: true, ignoreTypeReferences: true },
      ],
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: { parserOptions: { parser: ts.parser } },
  },
);
