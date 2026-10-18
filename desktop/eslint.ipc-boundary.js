// The front-end IPC boundary (ADR-0001, src/ipc/README.md): only src/ipc/tauriTransport.ts may reach Tauri.
// Covered forms: static import / export-from / import type; dynamic import(); require(); import.meta.glob; any
// string or template literal naming an @tauri-apps package; the injected globals window.__TAURI__* (identifier,
// property and computed-literal access). Deliberate obfuscation (e.g. a computed property built at run time) cannot
// be detected statically; non-literal import() specifiers, eval and new Function are therefore banned outright.
// The permanent positive/negative cases are in src/ipc/boundary.lint.test.ts.

const HOST = 'Talk to the Rust host only through src/ipc (createHostApi / useHost).';

export const TAURI_GLOBALS = ['__TAURI__', '__TAURI_INTERNALS__', '__TAURI_IPC__', '__TAURI_METADATA__'];

export const ipcBoundaryRules = {
  'no-restricted-imports': ['error', { patterns: [{ group: ['@tauri-apps/*'], message: HOST }] }],
  'no-restricted-syntax': [
    'error',
    { selector: 'ImportExpression[source.value=/^@tauri-apps/]', message: HOST },
    {
      selector: "ImportExpression:not([source.type='Literal'])",
      message: 'import() needs a literal module specifier so the IPC boundary can be checked.',
    },
    { selector: "CallExpression[callee.name='require']", message: `ES modules only; no require(). ${HOST}` },
    {
      selector: 'MemberExpression[property.name=/^glob/]:has(MetaProperty)',
      message: `import.meta.glob is not used in this code base. ${HOST}`,
    },
    {
      // import.meta only as import.meta.env / .url / .hot, so it cannot be aliased to reach .glob.
      selector: 'MetaProperty:not(MemberExpression[property.name=/^(env|url|hot)$/] > MetaProperty)',
      message: 'Use import.meta only as import.meta.env, import.meta.url or import.meta.hot.',
    },
    { selector: 'Literal[value=/^@tauri-apps/]', message: HOST },
    { selector: 'TemplateElement[value.raw=/@tauri-apps|__TAURI/]', message: HOST },
    { selector: 'Identifier[name=/^__TAURI/]', message: HOST },
    { selector: 'Literal[value=/^__TAURI/]', message: HOST },
  ],
  'no-restricted-globals': ['error', ...TAURI_GLOBALS.map((name) => ({ name, message: HOST }))],
  'no-eval': 'error',
  'no-new-func': 'error',
};

/** Rules switched off for the one file that implements the Tauri transport. */
export const ipcBoundaryExemption = Object.fromEntries(
  Object.keys(ipcBoundaryRules).map((rule) => [rule, 'off']),
);
