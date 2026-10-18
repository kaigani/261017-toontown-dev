"""Run the authorized auto spike once; observe window ownership without touching other windows."""
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
u = c.WinDLL('user32', use_last_error=True)
k = c.WinDLL('kernel32', use_last_error=True)
CALLBACK = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
u.EnumWindows.argtypes = [CALLBACK, w.LPARAM]
u.EnumChildWindows.argtypes = [w.HWND, CALLBACK, w.LPARAM]
u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
u.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
u.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
u.GetParent.argtypes = [w.HWND]
u.GetParent.restype = w.HWND
u.IsWindowVisible.argtypes = [w.HWND]
k.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
k.OpenProcess.restype = w.HANDLE
k.QueryFullProcessImageNameW.argtypes = [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)]
k.CloseHandle.argtypes = [w.HANDLE]

def info(hwnd):
    pid = w.DWORD()
    tid = u.GetWindowThreadProcessId(hwnd, c.byref(pid))
    cls, title = c.create_unicode_buffer(256), c.create_unicode_buffer(512)
    u.GetClassNameW(hwnd, cls, len(cls)); u.GetWindowTextW(hwnd, title, len(title))
    image = c.create_unicode_buffer(2048); size = w.DWORD(len(image))
    proc = k.OpenProcess(0x1000, False, pid.value)
    if proc:
        k.QueryFullProcessImageNameW(proc, 0, image, c.byref(size)); k.CloseHandle(proc)
    return dict(hwnd=hwnd, parent=u.GetParent(hwnd), pid=pid.value, tid=tid,
                cls=cls.value, title=title.value, process=image.value,
                visible=bool(u.IsWindowVisible(hwnd)))

def snapshot(pid):
    rows = []
    @CALLBACK
    def top(hwnd, _):
        v = info(hwnd)
        if v['pid'] == pid:
            rows.append(v)
            @CALLBACK
            def child(ch, _):
                rows.append(info(ch)); return True
            u.EnumChildWindows(hwnd, child, 0)
        elif v['cls'] == 'Shell_SystemDim' or v['process'].lower().endswith('pickerhost.exe'):
            rows.append(v)
        return True
    u.EnumWindows(top, 0)
    return rows

if __name__ == '__main__':
    with (OUT/'local-spike-stdout.txt').open('w') as stdout, (OUT/'local-spike-stderr.txt').open('w') as stderr:
        p = subprocess.Popen([str(ROOT/'target/release/toontown.exe'), '--spike', 'tablet',
                              '--spike-log', str(OUT/'local-tablet.json'), '--spike-auto'],
                             cwd=ROOT, stdout=stdout, stderr=stderr)
        records = []
        start = time.monotonic()
        while p.poll() is None and time.monotonic()-start < 130:
            records.append(dict(elapsed=time.monotonic()-start, windows=snapshot(p.pid)))
            time.sleep(.5)
        if p.poll() is None:
            p.terminate(); p.wait(timeout=10)
        result = dict(pid=p.pid, exitCode=p.returncode, elapsed=time.monotonic()-start, snapshots=records)
    (OUT/'windows-observation.json').write_text(json.dumps(result,indent=2),encoding='utf8')
    print('exit',p.returncode,'elapsed',result['elapsed'])
    if (OUT/'local-tablet.json').exists():
        log=json.loads((OUT/'local-tablet.json').read_text())
        for status in log['statusLog']: print(status['message'])
