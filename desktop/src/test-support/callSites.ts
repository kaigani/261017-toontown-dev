// The command calls of product code (ruling D-35; C43B-R2S-01): only executable call expressions count, found with a
// parser, never a regular expression over raw text. A call in a comment, the contents of a string, or code in a test or a
// test helper is not a call site.
//
// - TypeScript and TSX: the TypeScript compiler's syntax tree. A call site is a call expression `run('id', ...)`,
//   `x.run('id', ...)`, `perform('id', ...)`, `x.perform('id', ...)` or `useQuery(services, 'id', ...)` whose id
//   argument is a string literal, and each entry `name: ['id', params]` of the store's `SLICES` table (the queries the
//   store runs on every refresh).
// - Rust (the host): a lexer that skips comments, keeps string literals whole and stops at `#[cfg(test)]`. A call site is
//   the token sequence `.run("id"`, `q("id"`, `Invocation::user("id"` or the field `command: "id"`.
//
// Product code is `desktop/src` and `desktop/src-tauri/src` without generated code, tests (`*.test.ts(x)`, `test/`) and
// test helpers (`test-support/`, `__probes__/`, `testdata/`).

import ts from 'typescript';

/** The shape of a command id. */
const ID = /^[a-z][A-Za-z]*\.[A-Za-z][A-Za-z.]*[A-Za-z]$/;

/** Directories that hold no product code. */
export const NOT_PRODUCT_DIRS: readonly string[] = [
  'generated',
  'test',
  'test-support',
  '__probes__',
  'testdata',
  // The M0 diagnostics (`--ipc-bench`, `--timing-check`): absent from the default build (gating.build.test.ts).
  'diagnostics',
];

/** Whether a repository path (with `/`) is product code: a `.ts`, `.tsx` or `.rs` file outside tests and test helpers. */
export function isProductFile(path: string): boolean {
  if (!/\.(ts|tsx|rs)$/.test(path) || /\.test\.tsx?$/.test(path)) return false;
  return !path.split('/').some((part) => NOT_PRODUCT_DIRS.includes(part));
}

function literalId(node: ts.Node | undefined): string | null {
  if (!node) return null;
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node))
    return ID.test(node.text) ? node.text : null;
  return null;
}

function unwrap(node: ts.Expression): ts.Expression {
  let n = node;
  while (ts.isAsExpression(n) || ts.isSatisfiesExpression(n) || ts.isParenthesizedExpression(n))
    n = n.expression;
  return n;
}

/** The command ids a TypeScript or TSX source calls. */
export function tsCallSites(text: string, fileName: string): string[] {
  const kind = fileName.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  const source = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, false, kind);
  const out = new Set<string>();
  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      const name = ts.isPropertyAccessExpression(callee)
        ? callee.name.text
        : ts.isIdentifier(callee)
          ? callee.text
          : null;
      const id =
        name === 'run' || name === 'perform'
          ? literalId(node.arguments[0])
          : name === 'useQuery'
            ? literalId(node.arguments[1])
            : null;
      if (id) out.add(id);
    }
    if (
      ts.isVariableDeclaration(node) &&
      ts.isIdentifier(node.name) &&
      node.name.text === 'SLICES' &&
      node.initializer
    ) {
      const table = unwrap(node.initializer);
      if (ts.isObjectLiteralExpression(table)) {
        for (const p of table.properties) {
          if (ts.isPropertyAssignment(p)) {
            const entry = unwrap(p.initializer);
            if (ts.isArrayLiteralExpression(entry)) {
              const id = literalId(entry.elements[0]);
              if (id) out.add(id);
            }
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  return [...out].sort();
}

interface Token {
  kind: 'ident' | 'string' | 'punct';
  text: string;
}

/** Rust tokens without comments; a string literal's token holds its contents. Enough of Rust's lexical grammar for the
 *  call forms: line and nested block comments, strings with escapes, raw strings, byte strings, characters and
 *  lifetimes, identifiers, `::` and single punctuation. */
export function rustTokens(text: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  const n = text.length;
  const at = (k: number) => text.charAt(k);
  while (i < n) {
    const c = at(i);
    if (/\s/.test(c)) {
      i += 1;
    } else if (c === '/' && at(i + 1) === '/') {
      while (i < n && at(i) !== '\n') i += 1;
    } else if (c === '/' && at(i + 1) === '*') {
      let depth = 1;
      i += 2;
      while (i < n && depth > 0) {
        if (at(i) === '/' && at(i + 1) === '*') {
          depth += 1;
          i += 2;
        } else if (at(i) === '*' && at(i + 1) === '/') {
          depth -= 1;
          i += 2;
        } else i += 1;
      }
    } else if ((c === 'r' || (c === 'b' && at(i + 1) === 'r')) && /^b?r#*"/.test(text.slice(i, i + 40))) {
      // A raw string: r"...", r#"..."#, br"...".
      let j = i + (c === 'b' ? 2 : 1);
      let hashes = 0;
      while (at(j) === '#') {
        hashes += 1;
        j += 1;
      }
      j += 1;
      const close = `"${'#'.repeat(hashes)}`;
      const end = text.indexOf(close, j);
      const stop = end < 0 ? n : end;
      out.push({ kind: 'string', text: text.slice(j, stop) });
      i = stop + close.length;
    } else if (c === '"' || (c === 'b' && at(i + 1) === '"')) {
      let j = i + (c === 'b' ? 2 : 1);
      let value = '';
      while (j < n && at(j) !== '"') {
        if (at(j) === '\\') {
          value += at(j + 1);
          j += 2;
        } else {
          value += at(j);
          j += 1;
        }
      }
      out.push({ kind: 'string', text: value });
      i = j + 1;
    } else if (c === "'") {
      // A character literal ('a', '\n', '\'') or a lifetime ('a).
      if (at(i + 1) === '\\') {
        const end = text.indexOf("'", i + 3);
        i = end < 0 ? n : end + 1;
      } else if (at(i + 2) === "'") {
        i += 3;
      } else {
        i += 1;
        while (i < n && /[A-Za-z0-9_]/.test(at(i))) i += 1;
      }
    } else if (/[A-Za-z_]/.test(c)) {
      let j = i + 1;
      while (j < n && /[A-Za-z0-9_]/.test(at(j))) j += 1;
      out.push({ kind: 'ident', text: text.slice(i, j) });
      i = j;
    } else if (c === ':' && at(i + 1) === ':') {
      out.push({ kind: 'punct', text: '::' });
      i += 2;
    } else {
      out.push({ kind: 'punct', text: c });
      i += 1;
    }
  }
  return out;
}

/** Whether the tokens from `k` are `mod <name> { ... }` with nothing after its closing brace. */
function isTrailingTestModule(all: Token[], k: number): boolean {
  if (all[k]?.text !== 'mod' || all[k + 1]?.kind !== 'ident' || all[k + 2]?.text !== '{') return false;
  let depth = 0;
  for (let i = k + 2; i < all.length; i++) {
    const t = all[i];
    if (t?.kind !== 'punct') continue;
    if (t.text === '{') depth++;
    else if (t.text === '}') {
      depth--;
      if (depth === 0) return i === all.length - 1;
    }
  }
  return false;
}

/** The command ids a Rust source calls, before its `#[cfg(test)]` module. */
export function rustCallSites(text: string): string[] {
  const all = rustTokens(text);
  const tokens: Token[] = [];
  for (let k = 0; k < all.length; k++) {
    const seq = all.slice(k, k + 7).map((t) => t.text);
    if (seq.join(' ') === '# [ cfg ( test ) ]' && all[k + 2]?.kind === 'ident') {
      // Ruling D-48 item 7: reading stops only at the trailing test module (`#[cfg(test)] mod name { ... }` and nothing
      // after it). A `#[cfg(test)]` on anything else (a function, an item followed by product code) fails closed: the
      // file is refused, never read only up to that point.
      if (!isTrailingTestModule(all, k + 7)) {
        throw new Error('#[cfg(test)] marks something other than the trailing test module: refused');
      }
      break;
    }
    const token = all[k];
    if (token) tokens.push(token);
  }
  const out = new Set<string>();
  const is = (k: number, kind: Token['kind'], text?: string) => {
    const t = tokens[k];
    return t?.kind === kind && (text === undefined || t.text === text);
  };
  const id = (k: number) => {
    const t = tokens[k];
    return t?.kind === 'string' && ID.test(t.text) ? t.text : null;
  };
  for (let k = 0; k < tokens.length; k++) {
    let found: string | null = null;
    if (is(k, 'punct', '.') && is(k + 1, 'ident', 'run') && is(k + 2, 'punct', '(')) found = id(k + 3);
    else if (is(k, 'ident', 'q') && !is(k - 1, 'punct', '.') && is(k + 1, 'punct', '(')) found = id(k + 2);
    else if (
      is(k, 'ident', 'Invocation') &&
      is(k + 1, 'punct', '::') &&
      is(k + 2, 'ident', 'user') &&
      is(k + 3, 'punct', '(')
    )
      found = id(k + 4);
    else if (is(k, 'ident', 'command') && is(k + 1, 'punct', ':') && !is(k + 2, 'punct', ':'))
      found = id(k + 2);
    if (found) out.add(found);
  }
  return [...out].sort();
}

/** The command ids a product file calls (none for a file that is not product code). */
export function callSitesOf(path: string, text: string): string[] {
  if (!isProductFile(path)) return [];
  return path.endsWith('.rs') ? rustCallSites(text) : tsCallSites(text, path);
}
