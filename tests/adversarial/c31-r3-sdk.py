"""Final-round official SDK review. Default tests helpers; explicit paths preserve raw receipts."""
import asyncio
import importlib.metadata
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
HEAD = "9d42034ab37cc2c6e904d0e4bceba3fe975a7d92"

def structured(result):
    return getattr(result, "structured_content", None) or getattr(result, "structuredContent", None) or {}

def examples():
    result = []
    for name in ["element", "element_read", "element_bank", "cell", "xsheet"]:
        text = (ROOT / f"crates/executor/src/commands/{name}.rs").read_text(encoding="utf-8")
        for block in text.split("#[command(")[1:]:
            declaration = block.split(")]", 1)[0]
            ident = re.search(r'id = "([^"]+)"', declaration)
            if not ident or "kind = event" in declaration:
                continue
            example = re.search(r'example\(title = "[^"]*", params = (?:r#+"(.*?)"#+|"(.*?)")\)', declaration, re.S)
            if not example:
                raise ValueError("no example: " + ident[1])
            raw = example[1] if example[1] is not None else json.loads('"' + example[2] + '"')
            result.append((ident[1], json.loads(raw)))
    return result

class Helpers(unittest.TestCase):
    def test_every_invocable_c31_command_has_an_example(self):
        rows = examples()
        self.assertEqual(len(rows), 59)
        self.assertEqual(len({r[0] for r in rows}), 59)

async def exercise(binary, base=None):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    if importlib.metadata.version("mcp") != "2.0.0":
        raise RuntimeError("mcp==2.0.0 required")
    calls, checks = [], []
    with tempfile.TemporaryDirectory(prefix="tt-c31-r3-sdk-") as temp:
        directory = pathlib.Path(temp)
        async def roots(_):
            allowed=[types.Root(uri=directory.as_uri(), name="review"), types.Root(uri=ROOT.as_uri(), name="review-worktree")]
            if base:allowed.append(types.Root(uri=base.parent.as_uri(),name="fixture"))
            return types.ListRootsResult(roots=allowed)
        params = StdioServerParameters(command=str(binary), args=["--headless", "--mcp-stdio", "--user-dir", str(directory / "user"), "--policy", "autonomous"])
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write, list_roots_callback=roots) as client:
                await client.initialize()
                async def run(command, p, dry=False):
                    envelope = structured(await client.call_tool("run_command", {"id": command, "params": p, "dryRun": dry}))
                    calls.append(dict(command=command, params=p, dryRun=dry, envelope=envelope))
                    return envelope
                async def job(command, p):
                    e = await run(command, p)
                    if e.get("status")!="ok":raise RuntimeError(f"job start failed: {command}: {e}")
                    if e.get("result", {}).get("jobId"):
                        result=structured(await client.call_tool("job_wait", {"jobId": e["result"]["jobId"], "timeoutMs": 60000}))
                        calls.append(dict(command="job_wait",envelope=result))
                        if 'succeeded' not in json.dumps(result):raise RuntimeError(f"job failed: {command}: {result}")
                        return result
                    return e
                async def events(after):
                    resource=await client.read_resource(f"app://events?after={after}")
                    body=json.loads(resource.contents[0].text)
                    calls.append(dict(command="resources/read events",after=after,result=body))
                    return body
                def check(name, ok, detail=None):
                    checks.append(dict(check=name, ok=bool(ok), detail=detail))
                await run("project.create", {"name": "R3 SDK"})
                e = await run("element.create", {"type": "drawing", "name": "Source928"})
                element = e["result"]["elementIds"][0]
                await run("doc.object.patch", {"targets": [element], "props": {"meta": {"r3": "old928"}}})
                e = await run("doc.object.patch", {"targets": [element], "ops": [{"op": "replace", "path": "/meta/r3", "value": "new928"}, {"op": "copy", "from": "/meta/r3", "path": "/note"}]})
                e = await run("doc.object.get", {"ref": element})
                check("ordered ordinary-source copy", e["result"]["object"].get("note") == "new928", e)
                if base:
                    await job("project.saveAs",{"path":str(directory/"initial.animpkg")})
                    replay_base=directory/"replay-base.animpkg"
                    shutil.copyfile(base,replay_base)
                    def exported(v):
                        if isinstance(v,dict):return {k:exported(x) for k,x in v.items() if k not in ("exportedAt","bytes")}
                        if isinstance(v,list):return [exported(x) for x in v]
                        return v
                    for command, p in examples():
                        await job("project.open", {"path": str(base)})
                        # Same fixture and clipboard precondition as the committed example runner.
                        await run("cell.clipboard.set", {"elementIds": ['scene:"Intro"/element:"Bob"'], "from": 1, "to": 8})
                        before = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                        cursor=(await events(0))["lastSeq"]
                        dry = await run(command, p, True)
                        dry_after = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                        check(command + " dry document isolation", exported(before.get("result")) == exported(dry_after.get("result")), dry)
                        dry_events=await events(cursor)
                        check(command+" dry no doc event",not any(e["type"]=="doc.changed" for e in dry_events["events"]))
                        cursor=dry_events["lastSeq"]
                        actual = await run(command, p)
                        check(command + " example", actual.get("status") == "ok", actual)
                        if actual.get("status") != "ok":
                            continue
                        after = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                        accepted_events=await events(cursor)
                        if actual.get("undo"):
                            check(command+" one revision",actual["documentRev"]==dry["documentRev"]+1)
                            check(command+" one doc event",sum(e["type"]=="doc.changed" for e in accepted_events["events"])==1)
                        def content(v):
                            if isinstance(v, dict):
                                return {k: content(x) for k,x in v.items() if k not in ("rev", "documentRev", "modifiedAt", "bytes","exportedAt","createdAt")}
                            if isinstance(v, list): return [content(x) for x in v]
                            return v
                        replay_script=directory/(command+"-replay.json")
                        replay_script.write_text(json.dumps({"label":"R3 replay "+command,"atomic":False,"steps":[
                            {"id":"cell.clipboard.set","params":{"elementIds":['scene:"Intro"/element:"Bob"'],"from":1,"to":8}},
                            {"id":command,"params":p},
                            {"id":"doc.export.json","params":{"scope":"project","binaries":"omit"}}
                        ]}),encoding="utf-8")
                        replay_log=directory/(command+"-replay.jsonl")
                        process=await asyncio.to_thread(subprocess.run,[str(binary),"--headless","--project",str(replay_base),"--run",str(replay_script),"--out",str(replay_log),"--user-dir",str(directory/"replay-user"),"--policy","autonomous","--allow-path",str(directory)],capture_output=True,text=True)
                        lines=[json.loads(line) for line in replay_log.read_text(encoding="utf-8").splitlines()] if replay_log.exists() else []
                        calls.append(dict(command="--run replay",id=command,input=json.loads(replay_script.read_text()),exit=process.returncode,stdout=process.stdout,stderr=process.stderr,log=lines))
                        replay_results=[line["envelope"].get("result") for line in lines if line.get("record")=="step"]
                        check(command+" --run replay",process.returncode==0 and bool(replay_results) and content(replay_results[-1])==content(after.get("result")))
                        if exported(after.get("result")) != exported(before.get("result")) and actual.get("undo"):
                            history = await run("edit.history.get", {})
                            undo = await run("edit.undo", {})
                            restored = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                            # Revisions advance on undo: retain full raw JSON for review; compare content below.
                            check(command + " one undo", content(before.get("result")) == content(restored.get("result")), history)
                            await run("edit.redo", {})
                        saved = directory / (command + ".animpkg")
                        await job("project.saveAs", {"path": str(saved)})
                        a = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                        await run("project.close", {"onUnsaved": "discard"})
                        await job("project.open", {"path": str(saved)})
                        b = await run("doc.export.json", {"scope": "project", "binaries": "omit"})
                        check(command + " save/open identical", exported(a.get("result")) == exported(b.get("result")))
                check("worker available", (await run("doc.state.get", {})).get("status") == "ok")
    return dict(productHead=HEAD, sdkVersion="2.0.0", profile="release CI", binary=str(binary), calls=calls, checks=checks, allPassed=all(c["ok"] for c in checks))

if __name__ == "__main__":
    if len(sys.argv) in (5,7) and sys.argv[1] == "--binary" and sys.argv[3] == "--output":
        out = pathlib.Path(sys.argv[4]).resolve()
        if out.exists(): raise FileExistsError("preserve receipt")
        base = pathlib.Path(sys.argv[6]).resolve() if len(sys.argv)==7 and sys.argv[5]=="--base" else None
        report = asyncio.run(exercise(pathlib.Path(sys.argv[2]).resolve(),base))
        out.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
        print(json.dumps(dict(checks=len(report["checks"]), failed=[c["check"] for c in report["checks"] if not c["ok"]])))
        sys.exit(0 if report["allPassed"] else 1)
    unittest.main()
