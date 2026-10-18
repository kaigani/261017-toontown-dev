// The front-end IPC boundary, lint half (ADR-0001, src/ipc/README.md). The bundler half is ./vite-plugin.ts; both
// read the paths below. Rules:
//   * only src/ipc/tauriTransport.ts may reach Tauri: no @tauri-apps import in any form (static, type-only,
//     re-export, dynamic, require, import.meta.glob), no string/template literal naming the packages, no __TAURI*
//     globals, no eval / new Function, no non-literal import() specifiers;
//   * only src/ipc/index.ts may import src/ipc/tauriTransport.ts (UI code gets the validated HostApi, never the raw
//     transport);
//   * modules under src/ may import only within src/ (relative specifiers in canonical form, no absolute or scheme
//     specifiers, no backslashes); test files may additionally reach the repository's tests/ folder and
//     desktop/boundary/ (the definitions under test);
//   * no inline ESLint configuration of any kind takes effect (linterOptions.noInlineConfig, C01-R3-01): neither
//     `eslint-disable` nor `/* eslint rule: off */`; exceptions live in eslint.config.js. eslint-comments still
//     reports any disable directive as an error, so such comments cannot sit in the tree unnoticed.
// Applies to every JS/TS extension in the desktop package. Deliberate run-time obfuscation (e.g. a global name built
// from variables) cannot be detected statically; the bundler plugin scans transformed code for the Tauri globals
// and refuses to bundle anything it cannot place.
// Permanent cases: src/ipc/boundary.lint.test.ts, src/ipc/boundary.build.test.ts, tests/adversarial/c01-r2-*.

export const EXTENSIONS = ['ts', 'tsx', 'mts', 'cts', 'js', 'jsx', 'mjs', 'cjs'];
const EXT = `{${EXTENSIONS.join(',')}}`;

export const TRANSPORT_FILE = 'src/ipc/tauriTransport.ts';
export const IPC_INDEX_FILE = 'src/ipc/index.ts';
/** Files that legitimately contain bypass samples or the boundary's own patterns. Test-only; never bundled. */
export const SAMPLE_FILES = ['src/ipc/boundary.lint.test.ts', 'src/ipc/boundary.build.test.ts'];
export const TAURI_GLOBALS = ['__TAURI__', '__TAURI_INTERNALS__', '__TAURI_IPC__', '__TAURI_METADATA__'];
/** Deepest src/ subdirectory nesting covered by the per-depth import rules (a lint test enforces it). */
export const MAX_SRC_DEPTH = 12;

const HOST = 'Talk to the Rust host only through src/ipc (createHostApi / useHost).';
const RAW = 'Only src/ipc/index.ts may import the raw Tauri transport; use the validated HostApi.';
const OUTSIDE =
  'Modules under src/ may import only modules under src/ (tests: also <repo>/tests/ and desktop/boundary/).';

/** Import-like source literals: static import, export-from, export-star, dynamic import(). */
const SOURCE =
  ':matches(ImportDeclaration, ExportNamedDeclaration, ExportAllDeclaration, ImportExpression) > Literal';
// esquery regexes cannot contain '/', so path separators are written \x2F.
const up = (n) => `(\\.\\.\\x2F){${n}}`;
const SEGMENT = '[^.\\x2F\\\\][^\\x2F\\\\]*';
// `.` alone (the folder's index) is allowed; `..` alone is not (it could leave src/ without a trailing slash).
const CANONICAL_RELATIVE = `^(\\.|(\\.\\x2F|(\\.\\.\\x2F)+)(${SEGMENT}\\x2F)*${SEGMENT})$`;

const baseSyntax = [
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
    // import.meta only as import.meta.env / .url / .hot / .dirname / .filename, so it cannot be aliased to .glob.
    selector:
      'MetaProperty:not(MemberExpression[property.name=/^(env|url|hot|dirname|filename)$/] > MetaProperty)',
    message: 'Use import.meta only as import.meta.env, .url, .hot, .dirname or .filename.',
  },
  { selector: 'Literal[value=/^@tauri-apps/]', message: HOST },
  { selector: 'TemplateElement[value.raw=/@tauri-apps|__TAURI/]', message: HOST },
  { selector: 'Identifier[name=/^__TAURI/]', message: HOST },
  { selector: 'Literal[value=/^__TAURI/]', message: HOST },
  // Static imports of the transport are covered by no-restricted-imports (which src/ipc/index.ts is exempt from).
  { selector: 'ImportExpression > Literal[value=/tauriTransport/]', message: RAW },
  {
    selector: `${SOURCE}[value=/^(\\x2F|[A-Za-z]:|file:|data:|https?:|virtual:)/]`,
    message: 'No absolute, drive, file:, data:, http(s): or virtual: module specifiers.',
  },
  { selector: `${SOURCE}[value=/\\\\/]`, message: 'Module specifiers use forward slashes.' },
  {
    selector: `${SOURCE}[value=/^\\./]:not([value=/${CANONICAL_RELATIVE}/])`,
    message: 'Relative specifiers must be canonical: ./x or ../x with no ./ or ../ segments after a name.',
  },
];

// Shipped application code only (src/, not tests, not the transport), C01-R4-01/02:
//   * string-assembly APIs that can build a Tauri name the scanners cannot see in the source (the final-output scan
//     still catches whatever the minifier folds; `atob` is never folded, so lint is its only guard);
//   * workers, whose separate bundles do not run the boundary plugin (lifting this ban is ticketed to M1 C1.2).
const ASSEMBLY =
  'String assembly from fragments is not allowed in application code (IPC boundary, ADR-0001).';
const WORKERS =
  'Workers are not allowed until the IPC boundary covers worker builds (ADR-0001; ticket M1 C1.2).';
const productSyntax = [
  {
    selector:
      "CallExpression[callee.property.name='concat']:matches([callee.object.type='Literal'], [callee.object.type='TemplateLiteral'])",
    message: ASSEMBLY,
  },
  {
    selector: "CallExpression[callee.property.name='join'][callee.object.type='ArrayExpression']",
    message: ASSEMBLY,
  },
  { selector: 'TemplateLiteral > Literal', message: ASSEMBLY },
  { selector: "NewExpression[callee.name='URL']:has(MetaProperty)", message: WORKERS },
  { selector: `${SOURCE}[value=/[?&](worker|sharedworker)(&|=|$)/]`, message: WORKERS },
];
const productProperties = [
  ...['fromCharCode', 'fromCodePoint', 'raw'].map((property) => ({
    object: 'String',
    property,
    message: ASSEMBLY,
  })),
  ...['window', 'globalThis', 'self'].flatMap((object) => [
    { object, property: 'atob', message: ASSEMBLY },
    ...['Worker', 'SharedWorker', 'Worklet'].map((property) => ({ object, property, message: WORKERS })),
  ]),
  { object: 'navigator', property: 'serviceWorker', message: WORKERS },
];
const productGlobals = [
  { name: 'atob', message: ASSEMBLY },
  ...['Worker', 'SharedWorker', 'Worklet'].map((name) => ({ name, message: WORKERS })),
];

function restrictedImports({ allowTransport }) {
  const patterns = [{ group: ['@tauri-apps/*', '@tauri-apps'], message: HOST }];
  if (!allowTransport) patterns.push({ group: ['**/tauriTransport', '**/tauriTransport.*'], message: RAW });
  return ['error', { patterns }];
}

/** Rule names whose inline disabling is forbidden. */
export const BOUNDARY_RULE_NAMES = [
  'no-restricted-imports',
  'no-restricted-syntax',
  'no-restricted-globals',
  'no-restricted-properties',
  'no-eval',
  'no-new-func',
];

export const ipcBoundaryRules = {
  'no-restricted-imports': restrictedImports({ allowTransport: false }),
  'no-restricted-syntax': ['error', ...baseSyntax],
  'no-restricted-globals': ['error', ...TAURI_GLOBALS.map((name) => ({ name, message: HOST }))],
  'no-restricted-properties': 'off',
  'no-eval': 'error',
  'no-new-func': 'error',
};

/** Rules switched off for the transport implementation and the sample-holding boundary tests. */
export const ipcBoundaryExemption = Object.fromEntries(
  Object.keys(ipcBoundaryRules).map((rule) => [rule, 'off']),
);

/**
 * Builds the boundary part of the flat config. `commentsPlugin` is @eslint-community/eslint-plugin-eslint-comments.
 */
export function ipcBoundaryConfig(commentsPlugin) {
  const blocks = [
    {
      // Every JS/TS file of the desktop package.
      files: [`**/*.${EXT}`],
      plugins: { '@eslint-community/eslint-comments': commentsPlugin },
      linterOptions: { noInlineConfig: true },
      rules: {
        ...ipcBoundaryRules,
        '@eslint-community/eslint-comments/no-unlimited-disable': 'error',
        '@eslint-community/eslint-comments/no-restricted-disable': [
          'error',
          ...BOUNDARY_RULE_NAMES,
          '@eslint-community/eslint-comments/*',
        ],
      },
    },
  ];
  // Per-depth "stay inside src/" rules. A file at src/<d dirs>/f may climb at most d levels.
  for (let d = 0; d <= MAX_SRC_DEPTH; d++) {
    const dirs = '*/'.repeat(d);
    blocks.push({
      files: [`src/${dirs}*.${EXT}`],
      rules: {
        'no-restricted-syntax': [
          'error',
          ...baseSyntax,
          ...productSyntax,
          { selector: `${SOURCE}[value=/^${up(d + 1)}/]`, message: OUTSIDE },
        ],
      },
    });
    blocks.push({
      // Test files: d + 1 levels reach desktop/; from there only boundary/ or ../tests/ (the repository tests).
      files: [`src/${dirs}*.test.${EXT}`],
      rules: {
        'no-restricted-syntax': [
          'error',
          ...baseSyntax,
          {
            selector: `${SOURCE}[value=/^${up(d + 1)}(?!boundary\\x2F|\\.\\.\\x2Ftests\\x2F)/]`,
            message: OUTSIDE,
          },
        ],
      },
    });
  }
  blocks.push(
    {
      // Shipped application code: string-assembly and worker APIs (see productProperties / productGlobals).
      files: [`src/**/*.${EXT}`],
      ignores: [`src/**/*.test.${EXT}`],
      rules: {
        'no-restricted-properties': ['error', ...productProperties],
        'no-restricted-globals': [
          'error',
          ...TAURI_GLOBALS.map((name) => ({ name, message: HOST })),
          ...productGlobals,
        ],
      },
    },
    {
      files: [IPC_INDEX_FILE],
      rules: { 'no-restricted-imports': restrictedImports({ allowTransport: true }) },
    },
    { files: [TRANSPORT_FILE, ...SAMPLE_FILES, 'boundary/**'], rules: ipcBoundaryExemption },
  );
  return blocks;
}
