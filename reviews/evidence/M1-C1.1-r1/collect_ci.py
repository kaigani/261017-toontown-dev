"""Preserve all CI artifact bytes locally and commit small original evidence plus digests."""
import hashlib
import io
import json
from pathlib import Path
import subprocess
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LOCAL = ROOT / "target" / "review-m1-c11-r1-artifacts"
LOCAL.mkdir(parents=True, exist_ok=True)
artifacts = json.loads((HERE / "ci-artifacts.log").read_text(encoding="utf-8"))["artifacts"]
inventory = []
for a in artifacts:
    archive = LOCAL / (a["name"] + ".zip")
    if not archive.exists():
        data = subprocess.check_output(["gh", "api", a["archive_download_url"]], cwd=ROOT)
        archive.write_bytes(data)
    data = archive.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    assert a["digest"] == "sha256:" + sha
    entries = []
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for n in z.namelist():
            if n.endswith("/"):
                continue
            b = z.read(n)
            entry = {"name": n, "bytes": len(b), "sha256": hashlib.sha256(b).hexdigest()}
            if n.endswith((".json", ".txt", ".log", ".csv")) and len(b) < 2_000_000:
                out = HERE / "ci-artifacts" / a["name"] / n
                out.parent.mkdir(parents=True, exist_ok=True)
                out.write_bytes(b)
                entry["preserved"] = out.relative_to(HERE).as_posix()
            entries.append(entry)
    inventory.append({"artifact": a, "localArchive": str(archive), "verifiedSha256": sha, "entries": entries})
    print(a["name"], len(data), "verified", len(entries), "entries", flush=True)
(HERE / "ci-artifact-inventory.json").write_text(json.dumps(inventory, indent=2)+"\n", encoding="utf-8")
