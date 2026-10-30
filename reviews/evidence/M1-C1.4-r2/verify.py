"""Recompute retained evidence checks. Run from any directory; no product mutations."""
import collections
import hashlib
import json
import pathlib
import re
import subprocess
import zipfile

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT).decode('utf-8', errors='replace')
def totals(text):
    values = re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
    return dict(zip(('passed', 'failed', 'ignored'), map(sum, zip(*(map(int, v) for v in values))))) if values else {}

receipt_dir = ROOT / 'reviews/evidence/M1-C1.4'
receipt = json.loads((receipt_dir / 'receipt.json').read_text(encoding='utf-8'))
receipt_checks = {p: sha(receipt_dir / p) == value for p, value in receipt['sha256'].items()}
artifact_checks = {}
for platform in ('Windows', 'macOS'):
    base = ROOT / 'target/review-ci-verify' / ('headless-' + platform)
    previous = HERE / 'ci' / base.name
    for p in base.rglob('*'):
        if p.is_file():
            rel = p.relative_to(base)
            old = (previous / rel)
            if old.exists():
                old_hash = sha(old)
            else:
                with zipfile.ZipFile(HERE / 'ci-artifacts.zip') as z:
                    old_hash = hashlib.sha256(z.read(f'{base.name}/{rel.as_posix()}')).hexdigest()
            artifact_checks[f'{base.name}/{rel.as_posix()}'] = {
                'sha256': sha(p), 'matchesPriorDownload': sha(p) == old_hash}
review_paths = ['tests/adversarial', 'crates/mcp/tests/c14_r1_*', 'bins/headless/tests/c14_r1.rs',
                'reviews/M1-C1.4-r1.md', 'reviews/evidence/M1-C1.4-r1', 'crates/*/tests/*_r[0-9]*',
                'reviews/evidence/M1-C1.3-r2/inspection-memory.json', 'reviews/evidence/M1-C1.3-r3/open-latency.json']
diff = git('diff', '278c896..74fe3a9', '--', *review_paths)
(HERE / 'reviewer-path-diff.txt').write_text(diff, encoding='utf-8')
product_added = [line[1:] for line in git('diff', '--unified=0', '278c896..74fe3a9', '--', 'crates/*/src', 'bins/*/src', 'desktop/src').splitlines()
                 if line.startswith('+') and not line.startswith('+++')]
flagged = [x for x in product_added if re.search(r'\b(?:unwrap|expect)\s*\(|\b(?:panic!)|\bunsafe\b|#\[allow', x)]
(HERE / 'product-added-lint-candidates.json').write_text(json.dumps(flagged, indent=2), encoding='utf-8')
report = {'productCommit': '74fe3a956ea1c9a044095fdd3e17bb48520a1da5',
          'receipt': {'baseCommit': receipt['baseCommit'], 'logsMatch': receipt_checks,
                      'statuses': receipt['status'], 'workspaceTotals': totals((receipt_dir / 'gates/test.log').read_text(encoding='utf-8'))},
          'freshCIArtifactComparison': artifact_checks,
          'reviewerOwnedBuilderDiffEmpty': not diff,
          'dependencyDiff': git('diff', '278c896..74fe3a9', '--', 'Cargo.lock', 'deny.toml'),
          'ticketBodiesUnchangedSinceIntroduction': git('show', '5983f29:crates/mcp/tests/c14_r1_16_tickets.rs').replace('\r\n', '\n') == (ROOT / 'crates/mcp/tests/c14_tickets.rs').read_text(encoding='utf-8')}
(HERE / 'verification.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'receiptMatches': all(receipt_checks.values()), 'gateTotals': report['receipt']['workspaceTotals'],
                  'artifactFiles': len(artifact_checks), 'allArtifactsMatch': all(x['matchesPriorDownload'] for x in artifact_checks.values()),
                  'reviewerDiffEmpty': not diff, 'newLintCandidates': len(flagged)}, indent=2))
