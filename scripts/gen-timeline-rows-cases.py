"""Writes the case table of the rows the Timeline shows (ruling E-37), then copies it to the window
(desktop/src/timeline/, a byte copy kept equal by Vitest).

The rule, from the members `timeline.rows.list` declares (04 section 17.5): rows come in Timeline order, each with its
`depth` (0 at the top level, a child one deeper than its parent) and `collapsed`; a row is shown unless one of its
ancestors is collapsed. A collapsed row is itself shown when its ancestors allow it; its descendants are not.

The kernel (`timing_gesture::shown_rows`, used by the interpreter's hit-testing) and the window
(`desktop/src/timeline/rows.ts`, used to draw) both implement the rule and both check every case. This oracle states the
rule a third way: it finds each row's ancestors from the depths (a stack of the open parents) and asks whether any of
them is collapsed, where the two implementations scan in order and skip under the first collapsed row.

The cases: every well-formed depth sequence of one to five rows (the first at depth 0, each next row at most one deeper
than the row before it) with every combination of collapsed flags, then a few longer hand-written trees, and the empty
list.

Usage (from the repository root): python scripts/gen-timeline-rows-cases.py
"""
import itertools
import json
import pathlib
import shutil

ROOT = pathlib.Path(__file__).resolve().parents[1]
CASES = ROOT / "crates" / "executor" / "src" / "timeline-rows-cases.json"
WINDOW = ROOT / "desktop" / "src" / "timeline" / "timeline-rows-cases.json"


def shown(rows):
    """The indices of the rows shown: no ancestor collapsed."""
    out = []
    stack = []  # (depth, collapsed) of the open ancestors
    for i, (depth, collapsed) in enumerate(rows):
        while stack and stack[-1][0] >= depth:
            stack.pop()
        if not any(c for _, c in stack):
            out.append(i)
        stack.append((depth, collapsed))
    return out


def depth_sequences(n):
    def grow(seq):
        if len(seq) == n:
            yield list(seq)
            return
        for d in range(0, seq[-1] + 2):
            yield from grow(seq + [d])

    yield from grow([0])


def main():
    cases = [{"rows": [], "shown": []}]
    for n in range(1, 6):
        for depths in depth_sequences(n):
            for flags in itertools.product([False, True], repeat=n):
                rows = [[d, c] for d, c in zip(depths, flags)]
                cases.append({"rows": rows, "shown": shown(rows)})
    # Longer trees: a deep chain collapsed in the middle, siblings after a collapsed subtree, a collapsed leaf, two
    # top-level groups.
    hand = [
        [[0, False], [1, False], [2, False], [3, True], [4, False], [5, False], [4, False], [3, False], [2, False]],
        [[0, True], [1, False], [1, True], [2, False], [0, False], [1, False], [0, True]],
        [[0, False], [1, False], [1, False], [1, True], [0, False], [1, True], [2, True], [3, False], [1, False]],
        [[0, False]] * 12,
    ]
    for rows in hand:
        cases.append({"rows": rows, "shown": shown(rows)})
    doc = {
        "$comment": (
            "The rows the Timeline shows (ruling E-37): each case lists rows in Timeline order as [depth, collapsed] and "
            "the indices shown. A row is shown unless one of its ancestors is collapsed. Written by "
            "scripts/gen-timeline-rows-cases.py; checked by the kernel (timing_gesture::shown_rows) and the window "
            "(desktop/src/timeline/rows.ts)."
        ),
        "cases": cases,
    }
    # One case per line, so a diff names the case.
    body = ",\n".join(json.dumps(c, separators=(",", ":")) for c in cases)
    text = '{"$comment":' + json.dumps(doc["$comment"]) + ',"cases":[\n' + body + "\n]}\n"
    json.loads(text)
    CASES.write_text(text, encoding="utf-8", newline="\n")
    WINDOW.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(CASES, WINDOW)
    print(f"{len(cases)} cases -> {CASES.relative_to(ROOT)} and {WINDOW.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
