"""Reconcile final-run observations and the inherited review; no product writes."""
import collections
import json
import pathlib
import re

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]


def read(path):
    data = path.read_bytes()
    return data.decode('utf-16' if data.startswith((b'\xff\xfe', b'\xfe\xff')) else 'utf-8-sig')


review_path = ROOT / 'reviews/M1-C1.4-r1.md'
review = read(review_path)
review = review.replace('16 P1, 1 P2', '15 P1, 2 P2')
review = review.replace('### C14-R1-08 — P1:', '### C14-R1-08 — P2 ticket:')
note = (' **Final-run disposition:** downgraded from P1 to P2 under the completion brief: '
        'there is no reproducible runtime test of ownership loss during a suspended save at `3906001`. '
        'The existing hooks cannot suspend that interval, and adding a product hook is outside this '
        'review-only task. This remains a documented missing-proof ticket, not a demonstrated incorrect '
        'save result. No executable regression is claimed for 08; obligation 1 remains partially met.')
if '**Final-run disposition:**' not in review:
    review = review.replace('No speculative timing-based failing test was added.',
                            'No speculative timing-based failing test was added.' + note)
review = review.replace('The deterministic save-boundary hook is needed **now** to complete obligation 1.',
                        'The deterministic save-boundary hook is still needed to complete obligation 1; '
                        '08 is a P2 missing-proof ticket under this review completion rule.')
review = review.replace('local full workspace run and separate focused reruns are recorded.',
                        'local partial workspace attempts and separate focused reruns are recorded.')
review = review.replace('Fresh Windows tests reproduce the behavioral controls.',
                        'Fresh Windows tests reproduce the behavioral controls; the local builder '
                        'autosave measurement also failed with “no autosave” (see CI discussion).')
review = review.replace('The full workspace result, formatting, Clippy, front-end results and final evidence hashes are summarized by the final `verification.json`/receipt; no green baseline result is substituted for an unrun check.',
    'The inherited local workspace attempt was incomplete and failed '
    '`autosave_snapshot_latency_during_batches_queued_requests_and_large_media` with “no autosave”; '
    '`workspace-remainder.log` is incomplete too. These logs do not establish a green local workspace '
    'run. Baseline CI is green on both OSes; focused protocol autosave passes locally, including '
    'the final run. The builder autosave failure is retained as a timing/robustness risk, without '
    'inventing a new deterministic finding. Formatting, Clippy and front-end logs are retained '
    'from the earlier runs; they were not rerun in the final verification.')
provenance = ('\n**Provenance.** Round 1 was produced across three delegated runs on the same model, '
              'Codex GPT-6.1-Sol, after two provider capacity failures. The inherited report and tests '
              'were verified end to end in the final run: every reproducible finding was rerun, '
              '08 was explicitly downgraded for its missing runtime repro, the Cargo integration '
              'and CI Python discovery paths were exercised, and evidence analysis was regenerated. '
              'Baseline CI and historical measurements are distinguished from final-run execution.\n')
if '**Provenance.**' not in review:
    review = review.replace('## CI, code review and provenance\n',
                            '## CI, code review and provenance\n' + provenance)
runtime = read(HERE / 'final-python-runtime.log')
failed = re.findall(r'^FAIL:\s+(test_\w+)', runtime, re.M)
assert len(failed) == 22 and 'FAILED (failures=22)' in runtime
assert 'ERROR:' not in runtime
limits = read(HERE / 'final-mcp.log')
assert 'test autosave_under_queued_protocol_calls_and_background_export ... ok' in limits
assert 'test saved_observe_policy_and_limits_survive_restart ... FAILED' in limits
assert 'test configured_batch_limit_applies_to_batch_script ... FAILED' in limits
headless = read(HERE / 'final-cargo-headless.log')
assert 'Ran 23 tests' in headless and 'FAILED (failures=22)' in headless
assert 'real_stdio_boundary_regressions ... FAILED' in headless
discovery = read(HERE / 'final-reviewer-suites.log')
assert '11/12 reviewer Python suites passed' in discovery
assert 'FAIL c14-r1-mcp.py' in discovery
blocks = re.split(r'^### (C14-R1-\d+) — ', review, flags=re.M)
coverage = {}
for i in range(1, len(blocks), 2):
    fid, body = blocks[i:i+2]
    if fid.endswith('-08'):
        coverage[fid] = {'level': 'P2', 'disposition': 'downgraded: missing deterministic runtime repro'}
        continue
    names = re.findall(r'`((?:ReleaseFeature\.)?test_\w+)`', body)
    names += [name for name in ['saved_observe_policy_and_limits_survive_restart',
                               'configured_batch_limit_applies_to_batch_script'] if name in body]
    assert names, fid
    for name in names:
        assert name.split('.')[-1] in failed or f'test {name} ... FAILED' in limits, (fid, name)
    coverage[fid] = {'level': body[:2], 'tests': names, 'result': 'expected failure reproduced'}
lock = read(ROOT / 'crates/persist/src/lock.rs')
enum = re.search(r'pub enum Boundary \{(.*?)\n\}', lock, re.S)[1]
boundaries = re.findall(r'^\s+(\w+),', enum, re.M)
assert len(boundaries) == 7 and not any('Save' in b for b in boundaries)
verification = {
    'baseline': '390600123f6af0fef7a2961375e7f6804a290940',
    'runtime': {'total': 23, 'failures': failed, 'errors': 0,
                'passes': ['test_project_resource_is_readable']},
    'cargoHeadless': {'expectedFailure': True, 'runtimeTotal': 23, 'runtimeFailures': 22},
    'cargoMcp': {'autosavePassed': True, 'limitsExpectedFailures': 2},
    'pythonDiscovery': {'total': 12, 'inheritedPassed': 11, 'expectedFailingSuite': 'c14-r1-mcp.py'},
    'findingCoverage': coverage,
    'saveProofGap': {'lockBoundaries': boundaries, 'runtimeReproAvailable': False},
    'environment': {'displayVariablesRemoved': True, 'suiteNetworkUse': False,
                    'notes': 'Python suite uses only stdlib, local pipes and filesystem; no SDK or network needed.'},
}
(HERE / 'final-verification.json').write_text(json.dumps(verification, indent=2), encoding='utf-8')
samples = json.loads((ROOT / 'target/evidence/c14-r1/autosave-protocol-raw.json').read_text())
def stats(values):
    values = sorted(values)
    return values[round((len(values)-1)*.5)], values[round((len(values)-1)*.99)], values[-1]
snapshot = stats([s['snapshotMs'] for s in samples])
residual = stats([s['maintenanceElapsedMs'] - sum(s[k] or 0 for k in
                  ['snapshotMs', 'heartbeatMs', 'watchMs']) for s in samples])
review = re.sub(r'snapshot p50 [\d.]+ ms, p99/max [\d.]+ ms; queue/dispatch residual p50 [\d.]+ ms, max [\d.]+ ms\.',
    f'snapshot p50 {snapshot[0]:.4f} ms, p99/max {snapshot[2]:.4f} ms; '
    f'queue/dispatch residual p50 {residual[0]:.4f} ms, max {residual[2]:.4f} ms.', review)
final_note = ('\nFinal-run evidence: `final-verification.json` maps each retained executable finding to '
              'its observed assertion failure. The Python suite ran 23 tests: 22 expected failures, '
              'one passing project-resource check, zero test errors. Cargo headless reproduced the '
              'same failures; Cargo MCP passed the autosave measurement and failed both policy/limit '
              'regressions as expected. CI Python discovery passed all 11 inherited suites and failed '
              'only the new suite (its two static checks fail; runtime cases are skipped without '
              '`TT_REVIEW_BINARY`). No assertion was weakened.\n')
if 'Final-run evidence:' not in review:
    review = review.replace('## Merge and M2 risks', final_note + '\n## Merge and M2 risks')
review_path.write_text(review, encoding='utf-8')
print(json.dumps({'findings': collections.Counter(v['level'] for v in coverage.values()),
                  'snapshot': snapshot, 'residual': residual}, indent=2))
