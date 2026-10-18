## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/154.0.0.0 Safari/537.36 Edg/154.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26100.33438], webview 154.0.4258.62, CPUs 4; DPR 1, viewport {'height': 677, 'width': 1024}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgpu | groups1k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgpu | brush10k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgl2 | paths100k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 8.70 | 60.0 / 3047.7 / 3047.7 | 24 | 2.84 / 5.38 / 5.38 | 129.4 | 42.9 / 42.9 | 15.63 | 0.8 | 1437.50 / 1468.70 / 1468.70 | 9 of 9 | 0.10 / 0.20 / 0.20 | - | missed | 16.7 |
| webgl2 | groups1k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 2.02 | 16.5 / 230.6 / 230.6 | 53 | 0.72 / 12.75 / 12.75 | 54.2 | 3.2 / 3.2 | 15.62 | 16.9 | 62.50 / 78.20 / 93.80 | 169 of 169 | 0.10 / 0.40 / 4.70 | - | missed | 60.6 |
| webgl2 | brush10k | ANGLE (Microsoft, Microsoft Basic Render Driver (0x0000008C) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1008x445 | 10.09 | 68.3 / 89.2 / 89.2 | 143 | 2.99 / 4.00 / 4.00 | 390.9 | 16.0 / 16.0 | 15.62 | 8.9 | 109.40 / 140.70 / 156.20 | 89 of 89 | 0.10 / 0.50 / 4.20 | - | missed | 14.6 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu is not available | | | | | |
| webgl2 | 1008x445 | 1.79 | 16.1 / 18.3 / 21.7 | 0.1 / 0.4 / 1.1 | 109 | 16.2 |
