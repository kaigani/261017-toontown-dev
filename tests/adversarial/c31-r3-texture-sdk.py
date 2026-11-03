"""Release SDK texture recovery persistence probe; writes only an explicit new receipt."""
import asyncio
import importlib.metadata
import json
import pathlib
import sys
import tempfile
import unittest

def structured(r):return getattr(r,"structured_content",None) or getattr(r,"structuredContent",None) or {}

async def exercise(binary,fixtures):
    from mcp import ClientSession,StdioServerParameters,types
    from mcp.client.stdio import stdio_client
    if importlib.metadata.version("mcp")!="2.0.0":raise RuntimeError("mcp==2.0.0 required")
    calls=[];checks=[]
    with tempfile.TemporaryDirectory(prefix="tt-c31-r3-texture-") as temp:
        directory=pathlib.Path(temp)
        async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=fixtures.as_uri(),name="fixtures"),types.Root(uri=directory.as_uri(),name="output")])
        params=StdioServerParameters(command=str(binary),args=["--headless","--mcp-stdio","--project",str(fixtures/"texture-41.animpkg"),"--user-dir",str(directory/"user"),"--policy","autonomous"])
        async with stdio_client(params) as (read,write):
            async with ClientSession(read,write,list_roots_callback=roots) as client:
                await client.initialize()
                async def run(command,p):
                    e=structured(await client.call_tool("run_command",{"id":command,"params":p}));calls.append(dict(command=command,params=p,envelope=e));return e
                async def job(command,p):
                    e=await run(command,p);assert e["status"]=="ok",e
                    if e.get("result",{}).get("jobId"):
                        e=await run("job.wait",{"jobId":e["result"]["jobId"],"timeoutMs":60000});assert "succeeded" in json.dumps(e),e
                    return e
                clips=[]
                for c in [41,209]:
                    if c!=41:await job("project.open",{"path":str(fixtures/f"texture-{c}.animpkg")})
                    await run("cell.clipboard.set",{"elementIds":['scene:"Intro"/element:"Bob"'],"from":1,"to":1})
                    clips.append((await run("edit.clipboard.get",{"includeData":True}))["result"])
                await run("project.create",{"name":"Texture release928"})
                made=await run("element.create",{"type":"drawing","name":"Texture928"});target=made["result"]["elementIds"][0]
                entries=[]
                for i,clip in enumerate(clips):
                    await run("edit.clipboard.set",{"content":clip})
                    pasted=await run("cell.clipboard.apply",{"elementId":target,"frame":1+i*3,"mode":"newDrawings"})
                    drawing=(await run("doc.object.get",{"ref":pasted["result"]["createdDrawingIds"][0],"depth":-1}))["result"]["object"]
                    swatch=drawing["colourRecovery"][0]["swatch"]
                    entries.append(next(iter(swatch["values"].values()))["image"])
                checks.append(dict(check="texture collision remaps references",ok=entries[0]!=entries[1],entries=entries))
                before=[await run("doc.blob.get",{"entry":entry}) for entry in entries]
                checks.append(dict(check="both texture blobs readable before save",ok=all(e["status"]=="ok" for e in before)))
                output=directory/"textures.animpkg";await job("project.saveAs",{"path":str(output)})
                await run("project.close",{})
                await job("project.open",{"path":str(output)})
                after=[await run("doc.blob.get",{"entry":entry}) for entry in entries]
                checks.append(dict(check="both texture blobs survive save/open",ok=all(e["status"]=="ok" for e in after),observed=after))
    return dict(profile="release CI 9d42034",sdkVersion="2.0.0",calls=calls,checks=checks)

if __name__=="__main__":
    if len(sys.argv)==7 and sys.argv[1]=="--binary" and sys.argv[3]=="--fixtures" and sys.argv[5]=="--output":
        output=pathlib.Path(sys.argv[6]).resolve()
        if output.exists():raise FileExistsError("preserve receipt")
        report=asyncio.run(exercise(pathlib.Path(sys.argv[2]).resolve(),pathlib.Path(sys.argv[4]).resolve()))
        output.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8");print(json.dumps(report["checks"]));sys.exit(int(any(not r["ok"] for r in report["checks"])))
    class Helpers(unittest.TestCase):
        def test_empty_result(self):self.assertEqual(structured(object()),{})
    unittest.main()
