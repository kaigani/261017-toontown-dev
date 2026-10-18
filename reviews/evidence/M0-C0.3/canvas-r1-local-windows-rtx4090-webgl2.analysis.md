## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: webgl2
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 52.0 / 56.7 / 56.7 | 166 | 2.68 / 3.30 / 3.30 | 98.1 | 43.5 / 43.5 | 16.67 | 60.0 | 16.70 / 17.00 / 17.30 | 0 of 599 | 0.10 / 0.20 / 0.50 | - | met | 19.2 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 13.6 / 16.8 / 16.8 | 141 | 0.63 / 0.97 / 0.97 | 24.2 | 2.3 / 2.3 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 600 | 0.10 / 0.20 / 1.40 | - | met | 73.5 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 70.0 / 87.3 / 87.3 | 144 | 3.96 / 4.95 / 4.95 | 255.0 | 14.8 / 14.9 | 16.68 | 60.0 | 16.70 / 16.90 / 17.40 | 0 of 599 | 0.10 / 0.20 / 0.40 | - | met | 14.3 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgl2 | 1904x757 | 5.77 | 38.5 / 43.7 / 43.7 | 0.3 / 0.5 / 1.5 | 151 | 38.8 |
