"""Preserve exact-SHA CI logs and original measurement artifacts, hash full archives."""
from pathlib import Path
import hashlib
import io
import json
import subprocess
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
REPO = "kaigani/261017-toontown-dev"
RUN = "39167094209"
CACHE = ROOT / "target/c11-r2-ci-artifacts"
CACHE.mkdir(parents=True, exist_ok=True)
(HERE / "ci").mkdir(exist_ok=True)
def gh(*args):
    return subprocess.check_output(["gh", *args], cwd=ROOT)
metadata = gh("api", f"repos/{REPO}/actions/runs/{RUN}/artifacts")
(HERE / "ci/artifacts-api.json").write_bytes(metadata)
(HERE / "ci/full.log").write_bytes(gh("run", "view", RUN, "--repo", REPO, "--log"))
inventory = []
for artifact in json.loads(metadata)["artifacts"]:
    archive = gh("api", f"repos/{REPO}/actions/artifacts/{artifact['id']}/zip")
    digest = hashlib.sha256(archive).hexdigest()
    if artifact.get("digest"):
        assert artifact["digest"] == "sha256:" + digest
    path = CACHE / (artifact["name"] + ".zip")
    path.write_bytes(archive)
    record = {"name": artifact["name"], "id": artifact["id"], "sha256": digest,
              "archive": str(path), "bytes": len(archive), "entries": []}
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        for item in z.infolist():
            if item.is_dir():
                continue
            data = z.read(item)
            entry = {"name": item.filename, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            if item.filename.endswith((".json", ".txt", ".py")):
                dst = HERE / "ci" / artifact["name"] / item.filename
                dst.parent.mkdir(parents=True, exist_ok=True)
                assert dst.resolve().is_relative_to((HERE / "ci").resolve())
                dst.write_bytes(data)
                entry["preserved"] = str(dst.relative_to(HERE))
            record["entries"].append(entry)
    inventory.append(record)
    print(artifact["name"], len(archive), "bytes", flush=True)
(HERE / "ci/inventory.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")
