"""Independent R3 controls/rates/focus audit; uses unchanged R2 raw-data audit, never product summaries."""
import gzip
import importlib.util
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("r2_audit", ROOT / "reviews/evidence/M0-C0.2-r2/audit-matrix.py")
r2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r2)
results = []
for n in (3, 4, 5):
    path = ROOT / f"reviews/evidence/M0-C0.2/tablet-spike-windows-runner-r2-matrix{n}.json.gz"
    log = json.loads(gzip.decompress(path.read_bytes()))
    result = r2.audit(path)
    result["matrix"] = n
    requests = {(r["request"]["left"], r["request"]["top"], r["request"]["width"], r["request"]["height"]) for r in log["runs"]}
    canvases = {tuple(r["canvasRect"][k] for k in ("left", "top", "width", "height")) for r in log["runs"]}
    assert requests == {(8, 58, 1012, 275)}
    assert canvases == {(8, 58, 1012, 311.109375)}
    result["requestRects"] = list(requests)
    result["canvasRects"] = list(canvases)
    sine_xy = None
    for run, row in zip(log["runs"], result["runs"]):
        report = run.get("report") or (run.get("probe") or {}).get("report")
        row["canvasRect"] = run["canvasRect"]
        if report is None:
            assert row["error"]
            continue
        assert report["screenRect"] == [16, 109, 1012, 275]
        row["hostEstimatorHz"] = report["frames"] * 1e9 / (report["contactEndNs"] - report["contactStartNs"])
        assert math.isclose(row["hostEstimatorHz"], report["achievedHz"], rel_tol=1e-9)
        row["focus"] = report["focus"]
        assert [f["when"] for f in row["focus"]] == ["start", "end"]
        start, end = row["focus"]
        row["focusEndpointChanges"] = {k: [start[k], end[k]] for k in start if k != "when" and start[k] != end[k]}
        assert not (set(row["focusEndpointChanges"]) - {"captureHwnd"})
        assert start["foregroundPid"] == start["ownPid"] and start["foregroundClass"] == "Tauri Window"
        assert start["focusPid"] != start["ownPid"] and start["focusClass"] == "Chrome_WidgetWin_1"
        assert (start["foregroundHwnd"] == start["targetHwnd"]) == (run["target"] == "webview")
        if report["pattern"] == "sine":
            xy = list(zip(report["screenX"], report["screenY"]))
            if sine_xy is None:
                sine_xy = xy
            assert xy == sine_xy
            assert row["repeatedPixelFrames"] == [92, 421, 604, 933]
            if "web" in row:
                assert row["web"]["repeatedMissing"] == [92, 421, 604, 933]
            elif not run["probe"]["listeners"]["rts"]:
                assert not row["wmPointer"]["missing"]
        if report["pattern"] == "pressureOnly":
            assert set(report["tiltX"]) == {20} and set(report["tiltY"]) == {-10}
            assert set(report["rotation"]) == {90}
            assert len(set(zip(report["screenX"], report["screenY"]))) == 1
            assert row.get("web", row.get("wmPointer"))["received"] == (1 if run["target"] == "webview" else 1024)
        assert row["raw"]["hidReports"] == row["raw"]["savedReports"] == 1046
        assert row["raw"]["savedBytes"] == 16736
        if "web" in row:
            assert row["nativePackets"] == 0
            assert row["web"]["allKinds"]["down"] == row["web"]["allKinds"]["up"] == 1
            samples = log["web"]["samples"][slice(*run["webRange"])]
            points = {int(s["pressure"]*1024)-1: s for s in samples if s["pressure"] > 0 and s["kind"] != "up"}
            diffs = [s["twist"]-report["rotation"][i] for i, s in points.items()]
            row["web"]["twistDifferences"] = {str(d): diffs.count(d) for d in sorted(set(diffs))}
            assert set(diffs) <= {0, -180}
            menus = log["web"].get("contextMenus")
            row["contextMenus"] = None if menus is None else sum(run["perfStart"] <= m["t"] <= run["perfEnd"] for m in menus)
        elif "rts" in row:
            assert row["rts"]["unionWithWM"] == 1024
        else:
            assert row["wmPointer"]["received"] == 1024
        if run["target"] == "nativeWindow":
            entries = [e for e in run["probe"]["listeners"]["wmPointer"]["entries"] if e["inContact"]]
            assert all(e["rotation"] == report["rotation"][e["pressure"]-1] for e in entries)
            assert all((e["tiltX"],e["tiltY"]) == (report["tiltX"][e["pressure"]-1],report["tiltY"][e["pressure"]-1]) for e in entries)
    results.append(result)
(OUT / "independent-matrix.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
for result in results:
    print("MATRIX", result["matrix"], "request", result["requestRects"], "canvas", result["canvasRects"])
    for r in result["runs"]:
        if "error" in r:
            print(r["label"], "FAILED", r["error"])
        else:
            print(r["label"], "Hz", round(r["hostEstimatorHz"], 6), round(r["injectorHzFirstToLast"], 6),
                  "frames", r.get("web", r.get("wmPointer"))["received"], "RTS", r.get("rts"),
                  "menus", r.get("contextMenus"), "focus endpoint changes", r["focusEndpointChanges"])
    print("latency", result["handlerLatencyMs"], "coalescing", result["coalescedCount"])
print("All asserted controls passed. Endpoint snapshots do not establish uninterrupted focus within a run.")
