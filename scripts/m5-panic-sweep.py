"""M5 C5.1a ruling E-21: lists the candidate sites of standard-library calls that panic on their arguments, in the
product files C5.1a added (whole, outside `#[cfg(test)]`) or edited (the lines added since the `m4` tag). Each site is
classified by hand in reviews/M5-C5.1a-r1-response.md section 2.3; the list is reviews/evidence/M5-C5.1a/r1-sweep-sites.txt.

Usage (from the repository root): python scripts/m5-panic-sweep.py > reviews/evidence/M5-C5.1a/r1-sweep-sites.txt"""
import re
import subprocess
import sys

NEW = [
    'crates/executor/src/commands/playback.rs',
    'crates/executor/src/commands/timeline.rs',
    'crates/executor/src/commands/transfer_options.rs',
    'crates/executor/src/commands/xsheet_view.rs',
    'crates/executor/src/timeline_layout.rs',
    'crates/executor/src/timing_gesture.rs',
]
EDITED = [
    'crates/engine/src/selftest.rs', 'crates/executor/src/commands/anim_transform.rs',
    'crates/executor/src/commands/app.rs', 'crates/executor/src/commands/cell.rs',
    'crates/executor/src/commands/edit_ops.rs', 'crates/executor/src/commands/element.rs',
    'crates/executor/src/commands/element_read.rs', 'crates/executor/src/commands/mod.rs',
    'crates/executor/src/commands/prefs.rs', 'crates/executor/src/commands/render.rs',
    'crates/executor/src/commands/timing.rs', 'crates/executor/src/cx.rs', 'crates/executor/src/help.rs',
    'crates/executor/src/host.rs', 'crates/executor/src/lib.rs', 'crates/executor/src/maintenance.rs',
    'crates/executor/src/persistence.rs', 'crates/executor/src/run.rs', 'crates/executor/src/store.rs',
]
PATTERNS = [
    ('range', re.compile(r'\.range\(')),
    ('clamp', re.compile(r'\.clamp\(')),
    ('split_at', re.compile(r'split_at')),
    ('copy_from_slice', re.compile(r'copy_from_slice')),
    ('div/rem', re.compile(r'(?<![/*])\s/\s(?!/)|\s%\s|div_euclid|rem_euclid|checked_div')),
    ('vec index op', re.compile(r'\.(remove|insert|swap|drain|splice|split_off|truncate)\(')),
    ('index/slice', re.compile(r'[A-Za-z0-9_)\]]\[(?!\])')),
    ('duration', re.compile(r'Duration::from_secs_f64|from_secs_f32|Instant::now\(\)\s*\+|\+\s*Duration')),
    ('step/chunks', re.compile(r'step_by\(|\.chunks\(|\.windows\(')),
]


def lines_new(path):
    text = open(path, encoding='utf-8').read().split('\n')
    out = []
    for i, line in enumerate(text, 1):
        if line.strip().startswith('#[cfg(test)]'):
            break
        out.append((i, line))
    return out


def lines_added(path):
    diff = subprocess.run(['git', 'diff', '-U0', 'm4', '--', path], capture_output=True, text=True, encoding='utf-8').stdout
    out = []
    n = 0
    in_test = False
    for line in diff.split('\n'):
        m = re.match(r'@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@', line)
        if m:
            n = int(m.group(1))
            continue
        if line.startswith('+') and not line.startswith('+++'):
            out.append((n, line[1:]))
            n += 1
        elif line.startswith(' '):
            n += 1
    return out


def scan(path, lines):
    for n, line in lines:
        code = line.split('//')[0]
        if not code.strip() or code.strip().startswith(('#[', '///', '//!')):
            continue
        for name, pat in PATTERNS:
            c = code
            if name == 'index/slice':
                c = re.sub(r'(vec!|json!|&|\bmut |#)\[', ' ', c)
                c = re.sub(r':\s*\[[^\]]*\]', ' ', c)
            if pat.search(c):
                print(f'{path}:{n}: [{name}] {line.strip()}')


for p in NEW:
    scan(p, lines_new(p))
for p in EDITED:
    scan(p, lines_added(p))
