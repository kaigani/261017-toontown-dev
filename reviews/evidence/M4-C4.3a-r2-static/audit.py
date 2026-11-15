"""Read-only source/CI receipt collector. No Cargo or product writes."""
from pathlib import Path
import datetime
import json
import subprocess


def run(*args):
    return subprocess.check_output(args, text=True, encoding="utf-8", errors="replace")


root = Path(__file__).resolve().parents[3]
runs = json.loads(run("gh", "run", "list", "--branch", "m4-desktop-shell", "--limit", "6",
                      "--json", "databaseId,headSha,status,conclusion"))
jobs = json.loads(run("gh", "run", "view", "43728082844", "--json", "jobs"))["jobs"]
summary = {
    "at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "branchHead": run("git", "rev-parse", "origin/m4-desktop-shell").strip(),
    "runs": runs,
    "baselineRun": {
        "id": 43728082844,
        "jobCount": len(jobs),
        "notSuccessful": [
            {"name": j["name"], "conclusion": j["conclusion"],
             "failedSteps": [s["name"] for s in j["steps"] if s["conclusion"] == "failure"]}
            for j in jobs if j["conclusion"] != "success"
        ],
    },
    "handoffExists": subprocess.run(
        ["git", "cat-file", "-e", "origin/m4-desktop-shell:reviews/M4-C4.3a-handoff.md"],
        capture_output=True).returncode == 0,
    "counts": {
        "c43Tests": sum(p.read_text().count("#[test]") for p in (root / "crates/executor/tests/c43").glob("*.rs")),
        "reviewerRustTests": (root / "crates/executor/tests/c43a_r1_review.rs").read_text().count("#[test]"),
        "corruptCorpus": len(list((root / "desktop/src/canvas/testdata/scene/corrupt").glob("*.json"))),
        "validCorpus": len(list((root / "desktop/src/canvas/testdata/scene/valid").glob("*.json"))),
    },
}
Path(__file__).with_name("ci-summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps(summary, indent=2))
print("Reviewer edit:", run("git", "diff", "--numstat", "ebf4db6", "b9d57ff2", "--", "crates/executor/tests/c43a_r1_review.rs"))
for commit in ["94d99fd2", "07af0c78", "a723253c", "0386d32c", "b9d57ff2"]:
    print(commit, run("git", "rev-parse", "--verify", commit + "^{commit}").strip())
