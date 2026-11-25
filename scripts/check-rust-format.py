"""Formatting gate of CI's `lint` job: `cargo fmt -p <package> --check` for every workspace package, failing on any
file that differs except the reviewer-owned files named below, which are merged unchanged and are not edited by the
builder (declared CI edit, M5 C5.1a round 1; drop an entry once its file is formatted).

Usage (from the repository root): python scripts/check-rust-format.py
"""
import json
import re
import subprocess
import sys

# Reviewer-owned files merged unformatted, and the round that brought each.
REVIEWER_FILES = {
    'crates/executor/tests/c51a_r1_part2.rs': 'M5 C5.1a round 1 (dynamic, part 2)',
    'crates/mcp/tests/c51a_r1_notices.rs': 'M5 C5.1a round 1 (dynamic, part 2)',
    'crates/executor/tests/c52a_r1_gestures.rs': 'M5 C5.2a round 1 (dynamic)',
    'crates/executor/tests/c52a_r1_history.rs': 'M5 C5.2a round 1 (dynamic)',
    'crates/executor/tests/c52a_r1_lighttable.rs': 'M5 C5.2a round 1 (dynamic)',
    'crates/executor/tests/c52a_r1_thumbnails.rs': 'M5 C5.2a round 1 (dynamic)',
    'crates/executor/tests/c52a_r2_cells.rs': 'M5 C5.2a round 2 (dynamic)',
    'crates/executor/tests/c52a_r2_fixes.rs': 'M5 C5.2a round 2 (dynamic)',
}


def normal(path):
    p = path.replace('\\', '/')
    p = re.sub(r'^//\?/', '', p)
    return p


def main():
    meta = subprocess.run(
        ['cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked'],
        capture_output=True, text=True, encoding='utf-8', check=True,
    )
    packages = [p['name'] for p in json.loads(meta.stdout)['packages']]
    bad = []
    skipped = set()
    for name in packages:
        r = subprocess.run(['cargo', 'fmt', '-p', name, '--check'], capture_output=True, text=True, encoding='utf-8')
        if r.returncode == 0:
            continue
        files = set()
        for line in (r.stdout + r.stderr).splitlines():
            m = re.match(r'Diff in (.+?):\d+:', line)
            if m:
                files.add(normal(m.group(1)))
        if not files:
            print(r.stdout + r.stderr)
            bad.append(f'{name}: cargo fmt failed without a diff')
            continue
        for f in sorted(files):
            owned = [k for k in REVIEWER_FILES if f.endswith(k)]
            if owned:
                skipped.add(owned[0])
            else:
                bad.append(f)
    for k in sorted(skipped):
        print(f'not checked (reviewer-owned, {REVIEWER_FILES[k]}): {k}')
    if bad:
        print('rustfmt differs in:')
        for b in bad:
            print(f'  {b}')
        return 1
    print(f'{len(packages)} packages formatted')
    return 0


if __name__ == '__main__':
    sys.exit(main())
