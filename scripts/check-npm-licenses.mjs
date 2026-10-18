// Licence gate for the JavaScript that ships inside the app bundle (production dependencies of every workspace
// package). Same policy as the Rust graph (ADR-0003): the allow-list is read from `deny.toml` `[licenses] allow`,
// the single list for both ecosystems. Development dependencies are build tools that are not distributed; they are
// listed for information only.
//
// Usage: node scripts/check-npm-licenses.mjs        (exit 1 on a violation)
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

// SPDX License List data (spdx-license-ids, CC0-1.0; spdx-exceptions, CC-BY-3.0): root dev dependencies, used only
// by this build-time gate and never shipped.
const require = createRequire(import.meta.url);
const canonical = (ids) => new Map(ids.map((id) => [id.toLowerCase(), id]));
const LICENSE_IDS = canonical([...require('spdx-license-ids/index.json'), ...require('spdx-license-ids/deprecated.json')]);
const EXCEPTION_IDS = canonical([...require('spdx-exceptions/index.json'), ...require('spdx-exceptions/deprecated.json')]);

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

// SPDX 2.3 Annex D operand grammar.
const IDSTRING = '[A-Za-z0-9.-]+';
const LICENSE_REF = new RegExp(`^(?:DocumentRef-${IDSTRING}:)?LicenseRef-${IDSTRING}$`);
const LICENSE_ID = new RegExp(`^(${IDSTRING})([+])?$`);
const EXCEPTION_ID = new RegExp(`^${IDSTRING}$`);

/**
 * Validates one licence operand and returns its canonical form, or throws. Accepted: an identifier from the SPDX
 * License List (matched case-insensitively, as Annex D requires) with at most one trailing `+`, or a
 * `[DocumentRef-x:]LicenseRef-y` reference. Everything else — stray `+`/`:`, lower-case operators, unknown ids,
 * `LicenseRef-` with an empty suffix, `MIT++` — is a syntax error, so the whole expression fails closed.
 */
export function licenseOperand(token, expression) {
  if (LICENSE_REF.test(token)) return token;
  const m = LICENSE_ID.exec(token);
  const id = m && LICENSE_IDS.get(m[1].toLowerCase());
  if (!id) throw new SyntaxError(`not an SPDX licence identifier: ${token} in ${expression}`);
  return m[2] ? `${id}+` : id;
}

/** Validates an exception after `WITH`: an identifier from the SPDX License Exceptions list. */
export function exceptionOperand(token, expression) {
  const id = EXCEPTION_ID.test(token ?? '') && EXCEPTION_IDS.get(token.toLowerCase());
  if (!id) throw new SyntaxError(`not an SPDX exception identifier: ${token ?? 'end of expression'} in ${expression}`);
  return id;
}

/**
 * Parses an SPDX licence expression (SPDX 2.3 Annex D) into a tree:
 *   or   := and ('OR' and)*        (lowest precedence)
 *   and  := with ('AND' with)*
 *   with := atom ('WITH' exception)?
 *   atom := '(' or ')' | licence-operand
 * Operators must be upper case, as the specification requires. Every operand is validated (see `licenseOperand`);
 * any invalid token anywhere makes the whole expression invalid, even behind an allowed OR branch.
 */
export function parse(expression) {
  const tokens = tokenize(expression);
  let i = 0;
  const peek = () => tokens[i];
  const take = () => tokens[i++];

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
    if (atom.license === undefined || atom.license.startsWith('LicenseRef-') || atom.license.includes(':')) {
      throw new SyntaxError(`WITH needs an SPDX licence identifier on its left: ${expression}`);
    }
    return { license: `${atom.license} WITH ${exceptionOperand(take(), expression)}` };
  }
  function parseAtom() {
    const t = take();
    if (t === '(') {
      const inner = parseOr();
      if (take() !== ')') throw new SyntaxError(`unbalanced parentheses: ${expression}`);
      return inner;
    }
    if (t === undefined || t === ')' || t === 'AND' || t === 'OR' || t === 'WITH') {
      throw new SyntaxError(`expected a licence, got ${t ?? 'end of expression'}: ${expression}`);
    }
    return { license: licenseOperand(t, expression) };
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
