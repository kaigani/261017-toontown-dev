"""Independent official mcp==2.0.0 probes of round-two mixed RFC operations and generic cell cuts.

Default is a pure helper test. --binary BINARY --output NEW_JSON runs the product, preserving failing checks.
"""
import asyncio
import importlib.metadata
import json
import pathlib
import sys
import tempfile
import unittest


def structured(result):
    return getattr(result, "structured_content", None) or getattr(result, "structuredContent", None) or {}


class Helpers(unittest.TestCase):
    def test_empty_result_is_an_empty_object(self):
        self.assertEqual(structured(object()), {})


async def exercise(binary):
    from mcp import ClientSession, StdioServerParameters, types
    from mcp.client.stdio import stdio_client
    if importlib.metadata.version("mcp") != "2.0.0":
        raise RuntimeError("mcp==2.0.0 required")
    calls, checks = [], []
    with tempfile.TemporaryDirectory(prefix="tt-c31-r2-sdk-") as temp:
        async def roots(_):
            return types.ListRootsResult(roots=[types.Root(uri=pathlib.Path(temp).as_uri(), name="review")])
        params = StdioServerParameters(command=str(binary), args=["--headless", "--mcp-stdio", "--user-dir", str(pathlib.Path(temp)/"user"), "--policy", "autonomous"])
        async with stdio_client(params) as (read, write):
            async with ClientSession(read, write, list_roots_callback=roots) as client:
                await client.initialize()
                async def run(command, params):
                    envelope = structured(await client.call_tool("run_command", {"id": command, "params": params}))
                    calls.append(dict(command=command, params=params, envelope=envelope))
                    return envelope
                def check(name, ok):
                    checks.append(dict(check=name, ok=bool(ok)))
                await run("project.create", {"name":"Round two SDK"})
                made = await run("element.create", {"type":"drawing", "name":"Variant731"})
                element = made["result"]["elementIds"][0]
                await run("element.note.set", {"elementId":element, "text":"before731"})
                r = await run("doc.object.patch", {"targets":[element], "ops":[{"op":"test", "path":"/note", "value":"before731"}, {"op":"replace", "path":"/note", "value":"after731"}]})
                check("C31-R2-01 RFC test precedes delegated replace", r.get("status")=="ok")
                await run("element.current.set", {"elementId":element})
                await run("element.visibility.set", {"elementIds":[element], "scope":"scene", "visible":False})
                await run("doc.object.patch", {"targets":[element], "ops":[{"op":"copy", "from":"/visibleInScene", "path":"/visibleInXsheet"}]})
                r = await run("cell.selection.get", {})
                check("C31-R2-02 RFC copy advances hidden current column", r["result"]["currentElementId"]!=element)
                await run("element.visibility.set", {"elementIds":[element], "scope":"xsheet", "visible":True})
                await run("element.drawing.create", {"elementId":element, "name":"731", "exposeFrom":1, "exposeTo":6})
                for frame,text in [(1,"cut731"),(4,"tail731")]:
                    await run("cell.note.set", {"elementId":element,"frame":frame,"text":text})
                await run("cell.select", {"elementIds":[element], "from":1, "to":2})
                await run("edit.cut", {"context":"xsheet"})
                r = await run("doc.object.get", {"ref":element})
                check("C31-R2-04 generic cut clears cell note", "1" not in r["result"]["object"]["cellNotes"])
                r = await run("element.create", {"type":"clipping", "name":"海"*64})
                check("C31-R2-05 maximum name permits required parameter rows", r.get("status")=="ok")
                r = await run("doc.state.get", {})
                check("worker remains available", r.get("status")=="ok")
    return dict(sdkVersion="2.0.0", binary=str(binary), profile="debug", productHead="ffffc7387f03e2be125a634dd29f698598532d0d", calls=calls, checks=checks, allPassed=all(c["ok"] for c in checks))


if __name__ == "__main__":
    if len(sys.argv)==5 and sys.argv[1]=="--binary" and sys.argv[3]=="--output":
        output=pathlib.Path(sys.argv[4]).resolve()
        if output.exists():
            raise FileExistsError("preserve the prior receipt")
        report=asyncio.run(exercise(pathlib.Path(sys.argv[2]).resolve()))
        output.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
        print(json.dumps(report["checks"]))
        raise SystemExit(0 if report["allPassed"] else 1)
    unittest.main()
