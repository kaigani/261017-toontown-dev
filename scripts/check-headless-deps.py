"""Guards the claim that `toontown-headless` needs no display, window system or GPU (11 §3).

Two static checks:
  1. Crate graph: `cargo tree` of tt-headless (normal deps, host target) contains no GUI, webview, windowing or GPU
     crate (deny list below).
  2. Binary: the libraries the built executable imports DIRECTLY contain no GUI/GPU system library
     (Windows: PE import and delay-import tables, parsed here; macOS: `otool -L` load commands; Linux: `readelf -d`
     DT_NEEDED entries).

Scope, stated honestly: this is a deny-list guard against accidentally linking UI or graphics code. It does not see
libraries that a direct import loads transitively, nor anything loaded at run time with LoadLibrary/dlopen. The
runtime half of the proof is the CI job that runs `--selftest` on a Linux runner with no X/Wayland server at all,
plus `--selftest` on the Windows and macOS runners. Permanent negative controls: tests/adversarial/headless_guard.py
and bins/headless/tests/guard_fixtures.rs (real executables with a graphics import must fail this check).

Usage: python scripts/check-headless-deps.py <path-to-toontown-headless[.exe]>
Exit status 0 when both checks pass, 1 otherwise. Standard library only.
"""

import re
import struct
import subprocess
import sys

FORBIDDEN_CRATES = re.compile(
    r"^(tauri|tauri-.*|wry|tao|muda|webview2-com.*|objc2-app-kit|objc2-web-kit|objc2-quartz-core|objc2-metal"
    r"|cocoa|core-graphics|winit|wgpu.*|glow|glutin|gtk.*|gdk.*|webkit2gtk.*|softbuffer|x11.*|wayland-.*"
    r"|raw-window-handle|egui.*|iced.*|slint.*|sdl2.*|sdl3.*|ash|vulkano.*|metal|glium|minifb|pixels|fltk.*"
    r"|femtovg|skia.*|d3d12|gpu-allocator|naga|glfw.*|piston.*|druid.*|xilem.*|vello|vello_hybrid)$"
)

FORBIDDEN_WINDOWS_DLLS = {
    "user32.dll", "gdi32.dll", "gdiplus.dll", "comctl32.dll", "comdlg32.dll", "uxtheme.dll", "dwmapi.dll",
    "imm32.dll", "d2d1.dll", "dwrite.dll", "d3d9.dll", "d3d10.dll", "d3d11.dll", "d3d12.dll", "dxgi.dll",
    "opengl32.dll", "vulkan-1.dll", "webview2loader.dll", "shell32.dll",
    "dcomp.dll", "d3d10_1.dll", "win32u.dll", "d3dcompiler_47.dll", "dxcore.dll", "ddraw.dll", "glu32.dll",
    "d3d11on12.dll", "dinput8.dll", "xinput1_4.dll", "uiautomationcore.dll",
}
FORBIDDEN_MACOS = re.compile(
    r"/(AppKit|Cocoa|Metal|MetalKit|MetalPerformanceShaders|QuartzCore|CoreGraphics|WebKit|OpenGL|IOSurface"
    r"|CoreVideo|Carbon|SwiftUI|UIKit|GameController|SpriteKit|SceneKit|CoreImage)\.framework/"
)
FORBIDDEN_LINUX = re.compile(r"^lib(X11|Xext|Xrandr|Xi|xcb|xkbcommon|wayland-|GL|EGL|GLX|vulkan|drm|gbm|gtk|gdk|webkit2gtk|SDL)")


def crate_graph_violations():
    out = subprocess.run(
        ["cargo", "tree", "-p", "tt-headless", "-e", "normal", "--prefix", "none", "--format", "{p}"],
        check=True, capture_output=True, text=True,
    ).stdout
    names = {line.split()[0] for line in out.splitlines() if line.strip()}
    return sorted(n for n in names if FORBIDDEN_CRATES.match(n)), len(names)


def pe_imports(path):
    """DLL names from the import (dir 1) and delay-import (dir 13) tables of a PE32/PE32+ file."""
    data = open(path, "rb").read()
    (e_lfanew,) = struct.unpack_from("<I", data, 0x3C)
    if data[e_lfanew:e_lfanew + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    coff = e_lfanew + 4
    n_sections, = struct.unpack_from("<H", data, coff + 2)
    opt_size, = struct.unpack_from("<H", data, coff + 16)
    opt = coff + 20
    magic, = struct.unpack_from("<H", data, opt)
    dirs = opt + (112 if magic == 0x20B else 96)
    sections = []
    for i in range(n_sections):
        s = opt + opt_size + 40 * i
        vsize, va, rawsize, rawptr = struct.unpack_from("<IIII", data, s + 8)
        sections.append((va, max(vsize, rawsize), rawptr))

    def offset(rva):
        for va, size, raw in sections:
            if va <= rva < va + size:
                return rva - va + raw
        raise ValueError(f"RVA {rva:#x} outside sections")

    def cstr(rva):
        o = offset(rva)
        return data[o:data.index(b"\0", o)].decode("ascii").lower()

    names = []
    imp_rva, _ = struct.unpack_from("<II", data, dirs + 8 * 1)
    if imp_rva:
        o = offset(imp_rva)
        while True:
            desc = struct.unpack_from("<IIIII", data, o)
            if not any(desc):
                break
            names.append(cstr(desc[3]))
            o += 20
    delay_rva, _ = struct.unpack_from("<II", data, dirs + 8 * 13)
    if delay_rva:
        o = offset(delay_rva)
        while True:
            desc = struct.unpack_from("<IIIIIIII", data, o)
            if not any(desc):
                break
            names.append(cstr(desc[1]))
            o += 32
    return names


def linked_libraries(path):
    if sys.platform == "win32":
        return pe_imports(path)
    if sys.platform == "darwin":
        out = subprocess.run(["otool", "-L", path], check=True, capture_output=True, text=True).stdout
        return [line.strip().split(" (")[0] for line in out.splitlines()[1:] if line.strip()]
    out = subprocess.run(["readelf", "-d", path], check=True, capture_output=True, text=True).stdout
    return re.findall(r"Shared library: \[([^\]]+)\]", out)


def binary_violations(libs):
    if sys.platform == "win32":
        return sorted(l for l in libs if l in FORBIDDEN_WINDOWS_DLLS)
    if sys.platform == "darwin":
        return sorted(l for l in libs if FORBIDDEN_MACOS.search(l))
    return sorted(l for l in libs if FORBIDDEN_LINUX.match(l))


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    crates, n_crates = crate_graph_violations()
    libs = linked_libraries(sys.argv[1])
    bad_libs = binary_violations(libs)
    print(f"crate graph: {n_crates} crates, forbidden: {crates or 'none'}")
    print(f"linked libraries ({sys.platform}): {', '.join(libs)}")
    print(f"forbidden libraries: {bad_libs or 'none'}")
    ok = not crates and not bad_libs
    print("headless dependency check " + ("passed" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
