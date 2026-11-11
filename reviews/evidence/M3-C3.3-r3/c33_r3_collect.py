"""Summarize completed round-three receipts without changing historical evidence."""
from pathlib import Path
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent

def read(path):
    b = path.read_bytes()
    return b.decode("utf-16" if b.startswith(b"\xff\xfe") else "utf-8-sig", errors="replace")

def main():
    summaries = {}
    for path in sorted(HERE.glob("*.log")):
        text = read(path)
        totals = [tuple(map(int, m)) for m in re.findall(
            r"test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out", text)]
        roots = {}
        current = None
        for line in text.splitlines():
            if re.match(r"\s*(?:Running |Doc-tests )", line):
                current = line.strip()
            match = re.search(r"test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out", line)
            if match and current:
                roots[current] = tuple(map(int, match.groups()))
        exit_path = path.with_suffix(".exit")
        summaries[path.stem] = {
            "exit": read(exit_path).strip() if exit_path.exists() else "not-recorded",
            "totals": dict(zip(["passed", "failed", "ignored", "filtered"], map(sum, zip(*totals)))) if totals else {},
            "topLevelTotals": dict(zip(["passed", "failed", "ignored", "filtered"], map(sum, zip(*roots.values())))) if roots else {},
            "failedLines": re.findall(r"^test .+? \.\.\. FAILED\r?$", text, re.M),
        }
    summaries["trackedStatus"] = subprocess.check_output(["git", "status", "--short"], cwd=ROOT, text=True)
    (HERE / "execution-summary.json").write_text(json.dumps(summaries, indent=2) + "\n", encoding="utf-8")

if __name__ == "__main__":
    main()
