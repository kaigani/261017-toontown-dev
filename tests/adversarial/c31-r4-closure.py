"""Round-4 scoped closure via official mcp==2.0.0; explicit receipts must be outside the repository."""
import asyncio
import importlib.metadata
import json
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]

def structured(result):
    return getattr(result, "structured_content", None) or getattr(result, "structuredContent", None) or {}

def chain():
    return [
        {"op":"replace","path":"/meta/a","value":"new"},
        {"op":"copy","from":"/meta/a","path":"/note"},
        {"op":"test","path":"/note","value":"new"},
        {"op":"move","from":"/note","path":"/meta/c"},
        {"op":"remove","path":"/meta/a"},
        {"op":"copy","from":"/meta/a","path":"/name"},
    ]

async def exercise(binary):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    assert importlib.metadata.version("mcp") == "2.0.0"
    calls, checks = [], []
    with tempfile.TemporaryDirectory(prefix="tt-c31-r4-sdk-") as temp:
        directory = pathlib.Path(temp)
        async def roots(_):
            return types.ListRootsResult(roots=[types.Root(uri=directory.as_uri(),name="review")])
        params = StdioServerParameters(command=str(binary),args=["--headless","--mcp-stdio","--user-dir",str(directory/"user"),"--policy","autonomous"])
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write, list_roots_callback=roots) as client:
                await client.initialize()
                async def run(command, p):
                    e = structured(await client.call_tool("run_command",{"id":command,"params":p}))
                    calls.append(dict(command=command,params=p,envelope=e))
                    return e
                async def obj(ident):
                    return (await run("doc.object.get",{"ref":ident}))["result"]["object"]
                def check(name, passed):
                    checks.append(dict(check=name,ok=bool(passed)))
                assert (await run("project.create",{"name":"R4 closure"}))["status"] == "ok"
                ids=[]
                for name in ["Source","Second"]:
                    ids.append((await run("element.create",{"type":"drawing","name":name}))["result"]["elementIds"][0])
                await run("doc.object.patch",{"targets":ids,"props":{"meta":{"a":"old"}}})
                before=[await obj(i) for i in ids]
                history=await run("edit.history.get",{})
                state=await run("doc.state.get",{})
                # Prefix proves earlier delegated operations succeed before the final missing-source refusal.
                prefix=await run("doc.object.patch",{"targets":ids,"ops":chain()[:-1]})
                check("ordered prefix succeeds",prefix["status"]=="ok")
                for i in ids:
                    o=await obj(i)
                    check("prefix current copy move remove "+i,o["meta"].get("c")=="new" and "a" not in o["meta"])
                await run("edit.undo",{})
                before=[await obj(i) for i in ids]
                history=await run("edit.history.get",{})
                state=await run("doc.state.get",{})
                refused=await run("doc.object.patch",{"targets":ids,"ops":chain()})
                check("final source missing refuses",refused["status"]=="error" and refused["error"]["code"]=="arg.invalid")
                check("all targets unchanged",[await obj(i) for i in ids]==before)
                check("revision unchanged",refused["documentRev"]==state["documentRev"])
                check("history unchanged",(await run("edit.history.get",{}))["result"]==history["result"])
                moved=await run("doc.object.patch",{"targets":[ids[0]],"props":{"meta":{"source":"R4 props"}},"ops":[{"op":"move","from":"/meta/source","path":"/note"},{"op":"test","path":"/note","value":"R4 props"}]})
                o=await obj(ids[0])
                check("props source delegated move",moved["status"]=="ok" and o["note"]=="R4 props" and "source" not in o["meta"])
                await run("edit.undo",{})
                o=await obj(ids[0])
                check("props move one undo",all(o.get(k)==before[0].get(k) for k in ["meta","note"]))
    return dict(head="864b70d3d6da55179346d174abf2466ca825c90c",sdkVersion="2.0.0",binary=str(binary),calls=calls,checks=checks,allPassed=all(c["ok"] for c in checks))

class Helpers(unittest.TestCase):
    def test_chain_has_final_missing_source(self):
        self.assertEqual([o["op"] for o in chain()],["replace","copy","test","move","remove","copy"])
    def test_empty_structured(self):
        self.assertEqual(structured(object()),{})

if __name__ == "__main__":
    if len(sys.argv)==5 and sys.argv[1]=="--binary" and sys.argv[3]=="--output":
        output=pathlib.Path(sys.argv[4]).resolve()
        if output.is_relative_to(ROOT): raise ValueError("write receipts to a temporary directory")
        if output.exists(): raise FileExistsError(output)
        report=asyncio.run(exercise(pathlib.Path(sys.argv[2]).resolve()))
        output.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
        print(json.dumps(report["checks"]))
        sys.exit(int(not report["allPassed"]))
    unittest.main()
