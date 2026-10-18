## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36 Edg/154.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26100.33438], webview 154.0.4258.62, CPUs 4; DPR 1, viewport {'height': 677, 'width': 1024}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgpu | groups1k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgpu | brush10k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgl2 | paths100k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 8.70 | 106.8 / 2748.4 / 2748.4 | 23 | 4.46 / 5.94 / 5.94 | 186.7 | 81.7 / 81.7 | 15.62 | 0.6 | 1843.80 / 1890.60 / 1890.60 | 7 of 7 | 0.10 / 0.40 / 0.40 | - | missed | 9.4 |
| webgl2 | groups1k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 2.02 | 27.3 / 403.4 / 403.4 | 31 | 1.03 / 13.21 / 13.21 | 70.7 | 4.6 / 4.6 | 15.62 | 13.6 | 78.10 / 93.80 / 109.40 | 136 of 136 | 0.10 / 0.20 / 0.30 | - | missed | 36.6 |
| webgl2 | brush10k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 10.09 | 117.3 / 145.4 / 145.4 | 84 | 4.46 / 6.37 / 6.37 | 488.9 | 24.4 / 24.5 | 15.62 | 7.2 | 140.60 / 187.50 / 187.60 | 72 of 72 | 0.10 / 0.20 / 5.80 | - | missed | 8.5 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | |
| webgl2 | 1008x445 | 1.79 | 25.5 / 28.5 / 31.6 | 0.2 / 0.6 / 1.5 | 69 | 25.7 |
