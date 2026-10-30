"""Bounded child-process memory measurement for the permanent large-file inspection regression."""
import ctypes
import json
import os
import pathlib
import subprocess
import sys
import time

test_name=sys.argv[2] if len(sys.argv)>2 else "package_inspection_reads_a_bounded_header"
inspection=test_name=="package_inspection_reads_a_bounded_header"
env = dict(os.environ, TT_C13_MEMORY_CHILD="1")
started=time.monotonic()
p=subprocess.Popen([sys.argv[1],"--exact",test_name,"--nocapture"],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
peak=0
if sys.platform=="win32":
    class Counters(ctypes.Structure):
        _fields_=[("cb",ctypes.c_ulong),("PageFaultCount",ctypes.c_ulong)]+[(n,ctypes.c_size_t) for n in ["PeakWorkingSetSize","WorkingSetSize","QuotaPeakPagedPoolUsage","QuotaPagedPoolUsage","QuotaPeakNonPagedPoolUsage","QuotaNonPagedPoolUsage","PagefileUsage","PeakPagefileUsage"]]
    kernel=ctypes.WinDLL("kernel32",use_last_error=True)
    kernel.OpenProcess.restype=ctypes.c_void_p
    kernel.OpenProcess.argtypes=[ctypes.c_ulong,ctypes.c_int,ctypes.c_ulong]
    kernel.CloseHandle.argtypes=[ctypes.c_void_p]
    psapi=ctypes.WinDLL("psapi",use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes=[ctypes.c_void_p,ctypes.POINTER(Counters),ctypes.c_ulong]
    handle=kernel.OpenProcess(0x0410,False,p.pid)
    while p.poll() is None and time.monotonic()-started<20:
        c=Counters(); c.cb=ctypes.sizeof(c)
        if handle and psapi.GetProcessMemoryInfo(handle,ctypes.byref(c),c.cb): peak=max(peak,c.PeakWorkingSetSize)
        time.sleep(.005)
    if handle: kernel.CloseHandle(handle)
else:
    import resource
    while p.poll() is None and time.monotonic()-started<20: time.sleep(.01)
if p.poll() is None: p.kill()
output=p.communicate(timeout=5)[0]
if sys.platform!="win32":
    peak=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss*(1 if sys.platform=="darwin" else 1024)
result={"command":"doc.package.get validate=false" if inspection else test_name,"fileBytes":256<<20 if inspection else 64<<20,"peakWorkingSetBytes":peak,"seconds":time.monotonic()-started,"exitCode":p.returncode,"bounded":0<peak<(128<<20 if inspection else 384<<20)}
print(output.decode("utf-8",errors="replace"))
print(json.dumps(result))
if os.environ.get("TT_C13_R1_ARTIFACT_DIR"):
    pathlib.Path(os.environ["TT_C13_R1_ARTIFACT_DIR"],"inspection-memory.json" if inspection else "bomb-memory.json").write_text(json.dumps(result,indent=2),encoding="utf-8")
sys.exit(0 if result["bounded"] and p.returncode==0 else 1)
