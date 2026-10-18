## Canvas spike log (version 2)

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
| webgl2 | paths100k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 8.70 | 99.0 / 3164.5 / 3164.5 | 21 | 5.65 / 8.32 / 8.32 | 160.1 | 57.3 / 57.3 | 15.87 | 0.6 | 1812.60 / 1859.30 / 1859.30 | 7 of 7 | 0.10 / 0.30 / 0.30 | - | missed | 10.1 |
| webgl2 | groups1k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 2.02 | 25.9 / 402.2 / 402.2 | 32 | 1.35 / 24.82 / 24.82 | 62.7 | 3.9 / 4.0 | 15.62 | 13.4 | 78.10 / 93.80 / 109.40 | 134 of 134 | 0.10 / 0.50 / 0.60 | - | missed | 38.6 |
| webgl2 | brush10k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 10.09 | 112.1 / 143.6 / 143.6 | 87 | 6.32 / 7.18 / 7.18 | 475.9 | 17.4 / 17.5 | 15.87 | 7.2 | 140.70 / 171.90 / 187.60 | 73 of 73 | 0.10 / 0.60 / 0.70 | - | missed | 8.9 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | |
| webgl2 | 1008x445 | 1.79 | 23.6 / 28.0 / 29.8 | 0.2 / 0.6 / 1.3 | 74 | 23.8 |
