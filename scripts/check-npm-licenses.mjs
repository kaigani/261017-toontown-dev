// Licence gate for the JavaScript that ships inside the app bundle (production dependencies of every workspace
// package). Same policy as the Rust graph (ADR-0003): the allow-list is read from `deny.toml` `[licenses] allow`,
// the single list for both ecosystems. Development dependencies are build tools that are not distributed; they are
// listed for information only.
//
// Usage: node scripts/check-npm-licenses.mjs        (exit 1 on a violation)
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';

const DENY_TOML = fileURLToPath(new URL('../deny.toml', import.meta.url));

/** The `allow = [...]` array of the `[licenses]` table of deny.toml. */
export function readAllowList(tomlText = readFileSync(DENY_TOML, 'utf8')) {
  const table = /^\[licenses\][^\S\n]*\n([\s\S]*?)(?=^\[)/m.exec(tomlText + '\n[');
  const block = table && /^allow\s*=\s*\[([\s\S]*?)\]/m.exec(table[1]);
  if (!block) throw new Error('deny.toml has no [licenses] allow = [...] list');
  const ids = [...block[1].replace(/#.*$/gm, '').matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  if (ids.length === 0) throw new Error('deny.toml [licenses] allow list is empty');
  return new Set(ids);
}

export const ALLOWED = readAllowList();

/** Splits an SPDX expression into tokens: `(`, `)`, `AND`, `OR`, `WITH`, and licence/exception ids. */
export function tokenize(expression) {
  const tokens = [];
  const re = /\s*(\(|\)|[A-Za-z0-9.+:-]+)/y;
  let pos = 0;
  const text = expression.trim();
  while (pos < text.length) {
    re.lastIndex = pos;
    const m = re.exec(text);
    if (!m) throw new SyntaxError(`invalid character in licence expression at ${pos}: ${expression}`);
    tokens.push(m[1]);
    pos = re.lastIndex;
    while (pos < text.length && /\s/.test(text[pos])) pos++;
  }
  return tokens;
}

/**
 * Parses an SPDX licence expression (SPDX 2.3 Annex D) into a tree:
 *   or   := and ('OR' and)*        (lowest precedence)
 *   and  := with ('AND' with)*
 *   with := atom ('WITH' exception)?
 *   atom := '(' or ')' | licence-id ['+']
 * Operators must be upper case, as the specification requires; anything else is a syntax error.
 */
export function parse(expression) {
  const tokens = tokenize(expression);
  let i = 0;
  const peek = () => tokens[i];
  const take = () => tokens[i++];
  const isOperator = (t) => t === 'AND' || t === 'OR' || t === 'WITH';
  const isId = (t) => t !== undefined && t !== '(' && t !== ')' && !isOperator(t);

  function parseOr() {
    const terms = [parseAnd()];
    while (peek() === 'OR') {
      take();
      terms.push(parseAnd());
    }
    return terms.length === 1 ? terms[0] : { op: 'OR', terms };
  }
  function parseAnd() {
    const terms = [parseWith()];
    while (peek() === 'AND') {
      take();
      terms.push(parseWith());
    }
    return terms.length === 1 ? terms[0] : { op: 'AND', terms };
  }
  function parseWith() {
    const atom = parseAtom();
    if (peek() !== 'WITH') return atom;
    take();
    const exception = take();
    if (!isId(exception) || atom.license === undefined) {
      throw new SyntaxError(`WITH needs a licence on the left and an exception on the right: ${expression}`);
    }
    return { license: `${atom.license} WITH ${exception}` };
  }
  function parseAtom() {
    const t = take();
    if (t === '(') {
      const inner = parseOr();
      if (take() !== ')') throw new SyntaxError(`unbalanced parentheses: ${expression}`);
      return inner;
    }
    if (!isId(t)) throw new SyntaxError(`expected a licence id, got ${t ?? 'end of expression'}: ${expression}`);
    return { license: t };
  }

  if (tokens.length === 0) throw new SyntaxError('empty licence expression');
  const tree = parseOr();
  if (i !== tokens.length) throw new SyntaxError(`unexpected ${tokens[i]}: ${expression}`);
  return tree;
}

/** Evaluates a parsed tree: OR = any alternative satisfied, AND = every term satisfied. */
export function evaluate(tree, allowed = ALLOWED) {
  if (tree.op === 'OR') return tree.terms.some((t) => evaluate(t, allowed));
  if (tree.op === 'AND') return tree.terms.every((t) => evaluate(t, allowed));
  // `X+` ("X or later") is acceptable when X itself is allowed: the licensee may choose X.
  return allowed.has(tree.license) || (tree.license.endsWith('+') && allowed.has(tree.license.slice(0, -1)));
}

/** True when the expression is satisfiable using only allowed licences. Malformed expressions fail closed. */
export function satisfies(expression, allowed = ALLOWED) {
  try {
    return evaluate(parse(expression), allowed);
  } catch {
    return false;
  }
}

function licenses(prodOnly) {
  const args = ['--dir', 'desktop', 'licenses', 'list', '--json', ...(prodOnly ? ['--prod'] : ['--dev'])];
  const out = execFileSync(process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm', args, {
    encoding: 'utf8',
    shell: process.platform === 'win32',
  });
  return JSON.parse(out);
}

function main() {
  const violations = [];
  let count = 0;
  for (const [license, packages] of Object.entries(licenses(true))) {
    for (const pkg of packages) {
      count++;
      if (!satisfies(license)) violations.push(`${pkg.name}@${pkg.versions.join(',')}: ${license}`);
    }
  }
  const devSummary = Object.entries(licenses(false))
    .map(([license, packages]) => `${license} (${packages.length})`)
    .join(', ');

  console.log(`allow-list (from deny.toml): ${[...ALLOWED].join(', ')}`);
  console.log(`production packages checked: ${count}`);
  console.log(`development packages (not shipped, informational): ${devSummary}`);
  if (violations.length > 0) {
    console.error(`licence violations in shipped JavaScript:\n  ${violations.join('\n  ')}`);
    process.exit(1);
  }
  console.log('licences ok');
}

// Run only when executed directly, so tests can import the evaluator without invoking pnpm.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
