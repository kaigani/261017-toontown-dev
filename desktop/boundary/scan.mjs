// Shared scanner for the IPC boundary (C01-R3-01, C01-R4-01): finds mentions of the Tauri globals (`__TAURI*`) and
// package names (`@tauri-apps`) in source or emitted code, including disguised forms. Used by the Vite plugin
// (./vite-plugin.ts) on transformed modules, rendered chunks and the final minified output, and by the standalone
// post-build check (./check-dist.mjs) on the files written to disk. Plain JavaScript so Node can run it directly.
//
// Detected:
//   * literal text, case-insensitive (`__tauri`, `@tauri-apps`);
//   * escaped forms in the text: \xHH, \uHHHH, \u{H…}, &#NN; and &#xHH; are decoded before matching;
//   * String.fromCharCode / String.fromCodePoint calls with literal numbers, decoded;
//   * atob('…') with a literal argument, base64-decoded;
//   * in parsed code (AST): decoded string literals, template elements, identifiers, and constant string
//     expressions folded here: + of strings, templates with constant parts, 'a'.concat('b'), [..].join(sep),
//     String.fromCharCode/fromCodePoint, String.raw`…` and atob('…') with constant arguments.
// Names assembled from run-time data (variables, I/O) are out of scope by design (ADR-0001).
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import { parseAst } from 'vite';

export const TAURI_TEXT = /__tauri|@tauri-apps/i;
/** Emitted chunk that alone may contain Tauri code (vite.config.ts manualChunks). */
export const TRANSPORT_CHUNK = 'tauri-transport';

function decodeBase64(text) {
  try {
    return Buffer.from(text, 'base64').toString('latin1');
  } catch {
    return '';
  }
}

/** Decodes JS and HTML escapes so that `\x5f\x5fTAURI` or `&#95;&#95;TAURI` match. */
export function decodeEscapes(text) {
  return text
    .replace(/\\x([0-9a-f]{2})/gi, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/\\u\{([0-9a-f]{1,6})\}/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/\\u([0-9a-f]{4})/gi, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/&#x([0-9a-f]+);?/gi, (_, h) => String.fromCodePoint(parseInt(h, 16)))
    .replace(/&#(\d+);?/g, (_, d) => String.fromCodePoint(Number(d)));
}

/** Text-level mentions: plain, escaped, char-code sequences and base64 literals passed to atob. */
export function textMentions(text) {
  const hits = [];
  if (TAURI_TEXT.test(text)) hits.push('text');
  if (TAURI_TEXT.test(decodeEscapes(text))) hits.push('escaped text');
  for (const m of text.matchAll(/fromC(?:harCode|odePoint)\s*\(([^)]*)\)/gi)) {
    const codes = (m[1] ?? '').split(',').map((s) => Number(s.trim()));
    if (codes.length > 0 && codes.every((n) => Number.isInteger(n) && n >= 0 && n <= 0x10ffff)) {
      if (TAURI_TEXT.test(String.fromCodePoint(...codes))) hits.push('char codes');
    }
  }
  for (const m of text.matchAll(/atob\s*\(\s*(['"`])([A-Za-z0-9+/=\s]+)\1\s*\)/g)) {
    if (TAURI_TEXT.test(decodeBase64(m[2] ?? ''))) hits.push('base64');
  }
  return hits;
}

const isStringLiteral = (n) => n?.type === 'Literal' && typeof n.value === 'string';
const memberName = (n) =>
  n?.type === 'MemberExpression'
    ? n.computed
      ? isStringLiteral(n.property)
        ? n.property.value
        : undefined
      : n.property?.name
    : undefined;
const isStringObject = (n) => n?.type === 'Identifier' && n.name === 'String';

/** Folds a constant string expression, or returns undefined when it is not constant. */
export function constantString(node, depth = 0) {
  if (!node || depth > 64) return undefined;
  switch (node.type) {
    case 'Literal':
      return typeof node.value === 'string'
        ? node.value
        : typeof node.value === 'number'
          ? String(node.value)
          : undefined;
    case 'TemplateLiteral': {
      let out = '';
      for (let i = 0; i < node.quasis.length; i++) {
        out += node.quasis[i].value.cooked ?? node.quasis[i].value.raw;
        if (i < node.expressions.length) {
          const part = constantString(node.expressions[i], depth + 1);
          if (part === undefined) return undefined;
          out += part;
        }
      }
      return out;
    }
    case 'BinaryExpression': {
      if (node.operator !== '+') return undefined;
      const l = constantString(node.left, depth + 1);
      const r = constantString(node.right, depth + 1);
      return l !== undefined && r !== undefined ? l + r : undefined;
    }
    case 'TaggedTemplateExpression':
      // String.raw`…`
      if (
        isStringObject(node.tag?.object) &&
        memberName(node.tag) === 'raw' &&
        node.quasi.expressions.length === 0
      ) {
        return node.quasi.quasis.map((q) => q.value.raw).join('');
      }
      return undefined;
    case 'CallExpression': {
      const callee = node.callee;
      const args = node.arguments.map((a) => constantString(a, depth + 1));
      if (args.some((a) => a === undefined)) return undefined;
      const name = memberName(callee);
      if (isStringObject(callee?.object) && (name === 'fromCharCode' || name === 'fromCodePoint')) {
        const codes = args.map(Number);
        if (!codes.every((n) => Number.isInteger(n) && n >= 0 && n <= 0x10ffff)) return undefined;
        return String.fromCodePoint(...codes);
      }
      if (name === 'concat') {
        const base = constantString(callee.object, depth + 1);
        return base === undefined ? undefined : base + args.join('');
      }
      if (name === 'join' && callee.object?.type === 'ArrayExpression') {
        const parts = callee.object.elements.map((e) => constantString(e, depth + 1));
        if (parts.some((p) => p === undefined)) return undefined;
        return parts.join(args[0] ?? ',');
      }
      const isAtob = (callee?.type === 'Identifier' && callee.name === 'atob') || name === 'atob';
      if (isAtob && args.length === 1) return decodeBase64(args[0]);
      return undefined;
    }
    default:
      return undefined;
  }
}

/** AST-level mentions in parsed JavaScript, or null when the code does not parse. */
export function astMentions(code) {
  let program;
  try {
    program = parseAst(code);
  } catch {
    return null;
  }
  const hits = [];
  const stack = [program];
  while (stack.length > 0) {
    const node = stack.pop();
    if (typeof node !== 'object' || node === null) continue;
    if (Array.isArray(node)) {
      stack.push(...node);
      continue;
    }
    const type = node.type;
    if (type === 'Literal' && typeof node.value === 'string' && TAURI_TEXT.test(node.value))
      hits.push(node.value);
    if ((type === 'Identifier' || type === 'PrivateIdentifier') && TAURI_TEXT.test(node.name ?? '')) {
      hits.push(node.name);
    }
    if (type === 'TemplateElement' && TAURI_TEXT.test(node.value?.cooked ?? node.value?.raw ?? '')) {
      hits.push(node.value.cooked ?? node.value.raw);
    }
    if (
      type === 'CallExpression' ||
      type === 'BinaryExpression' ||
      type === 'TemplateLiteral' ||
      type === 'TaggedTemplateExpression'
    ) {
      const folded = constantString(node);
      if (folded !== undefined && TAURI_TEXT.test(folded) && !isStringLiteral(node))
        hits.push(`folded:${folded}`);
    }
    for (const [k, v] of Object.entries(node)) {
      if (k !== 'type' && typeof v === 'object' && v !== null) stack.push(v);
    }
  }
  return hits;
}

/** Every mention in a piece of emitted or source code (AST + text); `['unparseable']` if JS fails to parse. */
export function mentions(code, { javascript = true } = {}) {
  const text = textMentions(code);
  if (!javascript) return text;
  const ast = astMentions(code);
  return ast === null ? [...text, 'unparseable'] : [...text, ...ast];
}

/** Whether an emitted file name is the dedicated Tauri transport chunk. */
export function isTransportChunkFile(fileName) {
  return new RegExp(`(^|/)${TRANSPORT_CHUNK}(-[\\w-]+)?\\.m?js$`).test(fileName.replaceAll('\\', '/'));
}

/** Scans a build output directory on disk. Returns `{ file, hits }` for every offending .js/.mjs/.cjs/.html file. */
export function scanDist(dir) {
  const problems = [];
  let transportFiles = 0;
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      const full = path.join(d, name);
      if (statSync(full).isDirectory()) {
        walk(full);
        continue;
      }
      const rel = path.relative(dir, full).replaceAll('\\', '/');
      if (!/\.(m?js|cjs|html)$/.test(rel)) continue;
      if (isTransportChunkFile(rel)) {
        transportFiles++;
        continue;
      }
      const hits = mentions(readFileSync(full, 'utf8'), { javascript: !rel.endsWith('.html') });
      if (hits.length > 0) problems.push({ file: rel, hits });
    }
  };
  walk(dir);
  return { problems, transportFiles };
}
