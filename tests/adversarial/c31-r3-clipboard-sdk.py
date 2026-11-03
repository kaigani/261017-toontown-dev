"""Release SDK verification of project-ID collision and staged-byte capture. No implicit receipts."""
import asyncio
import importlib.metadata
import json
import pathlib
import sys
import tempfile
import unittest

def structured(result):return getattr(result,"structured_content",None) or getattr(result,"structuredContent",None) or {}

async def exercise(binary,source,destination):
    from mcp import ClientSession,StdioServerParameters,types
    from mcp.client.stdio import stdio_client
    if importlib.metadata.version("mcp")!="2.0.0":raise RuntimeError("mcp==2.0.0 required")
    calls=[];checks=[]
    with tempfile.TemporaryDirectory(prefix="tt-c31-r3-clipboard-") as temp:
        directory=pathlib.Path(temp)
        async def roots(_):return types.ListRootsResult(roots=[types.Root(uri=source.parent.as_uri(),name="source"),types.Root(uri=destination.parent.as_uri(),name="destination"),types.Root(uri=directory.as_uri(),name="output")])
        params=StdioServerParameters(command=str(binary),args=["--headless","--mcp-stdio","--project",str(source),"--user-dir",str(directory/"user"),"--policy","autonomous"])
        async with stdio_client(params) as (read,write):
            async with ClientSession(read,write,list_roots_callback=roots) as client:
                await client.initialize()
                async def run(command,p):
                    e=structured(await client.call_tool("run_command",{"id":command,"params":p}));calls.append(dict(command=command,params=p,envelope=e));return e
                async def job(command,p):
                    e=await run(command,p)
                    assert e["status"]=="ok",e
                    if e.get("result",{}).get("jobId"):
                        e=await run("job.wait",{"jobId":e["result"]["jobId"],"timeoutMs":60000});assert "succeeded" in json.dumps(e),e
                    return e
                bg='scene:"Intro"/element:"Background"'
                original=(await run("doc.object.get",{"ref":bg}))["result"]["object"]
                drawing=original["exposure"][0][0]
                image=(await run("doc.object.get",{"ref":drawing,"depth":-1}))["result"]["object"]["nodes"][0]["image"]
                await run("cell.clipboard.set",{"elementIds":[bg],"from":1,"to":1})
                await job("project.open",{"path":str(destination)})
                await run("cell.clipboard.apply",{"elementId":bg,"frame":29})
                obj=(await run("doc.object.get",{"ref":bg}))["result"]["object"]
                frame=0;placed=None
                for ident,length in obj["exposure"]:
                    frame+=length
                    if frame>=29:placed=ident;break
                actual=(await run("doc.object.get",{"ref":placed,"depth":-1}))["result"]["object"]["nodes"][0]["image"]
                checks.append(dict(check="cross-project colliding IDs keep source artwork",ok=actual["sha256"]==image["sha256"],expected=image,actual=actual))
                await job("project.saveAs",{"path":str(directory/"collision.animpkg")})
                await job("project.open",{"path":str(source)})
                await run("cell.clipboard.set",{"elementIds":[bg],"from":1,"to":1})
                original_clip=(await run("edit.clipboard.get",{"includeData":True}))["result"]
                await run("project.create",{"name":"Staged release928"})
                made=await run("element.create",{"type":"image","name":"Staged928"})
                target=made["result"]["elementIds"][0]
                batch={"label":"Paste copy928","atomic":True,"steps":[{"id":"cell.clipboard.apply","params":{"elementId":target,"frame":9}},{"id":"cell.clipboard.set","params":{"elementIds":[target],"from":9,"to":9}}]}
                e=structured(await client.call_tool("batch",batch));calls.append(dict(command="batch",params=batch,envelope=e));assert e["status"]=="ok",e
                copied=(await run("edit.clipboard.get",{"includeData":True}))["result"]
                checks.append(dict(check="atomic paste then copy captures staged bytes",ok=copied["payload"]["blobs"]==original_clip["payload"]["blobs"],expected=original_clip["payload"]["blobs"],actual=copied["payload"]["blobs"]))
    return dict(profile="release CI 9d42034",sdkVersion="2.0.0",calls=calls,checks=checks)

if __name__=="__main__":
    if len(sys.argv)==9 and sys.argv[1]=="--binary" and sys.argv[3]=="--source" and sys.argv[5]=="--destination" and sys.argv[7]=="--output":
        output=pathlib.Path(sys.argv[8]).resolve()
        if output.exists():raise FileExistsError("preserve receipt")
        report=asyncio.run(exercise(*(pathlib.Path(sys.argv[i]).resolve() for i in [2,4,6])))
        output.write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8");print(json.dumps(report["checks"]));sys.exit(int(any(not r["ok"] for r in report["checks"])))
    class Helpers(unittest.TestCase):
        def test_empty_structured_result(self):self.assertEqual(structured(object()),{})
    unittest.main()
