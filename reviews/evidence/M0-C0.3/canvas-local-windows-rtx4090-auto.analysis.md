## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 50.3 / 52.6 / 52.6 | 172 | 1.96 / 2.09 / 2.09 | 85.3 | 23.3 / 68.4 | 16.67 | 60.0 | 16.70 / 16.90 / 17.40 | 0 of 599 | 0.10 / 0.20 / 0.70 | 8.50 / 11.50 / 15.90 | met | 19.9 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 13.9 / 18.9 / 18.9 | 141 | 0.44 / 0.57 / 0.57 | 30.4 | 3.8 / 13.6 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.20 / 0.30 / 0.50 | 4.40 / 5.40 / 6.80 | met | 71.9 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 64.9 / 85.7 / 85.7 | 152 | 2.31 / 2.91 / 2.91 | 242.9 | 1.6 / 18.4 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 600 | 0.10 / 0.20 / 0.40 | 5.20 / 7.40 / 8.90 | met | 15.4 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 57.0 / 61.9 / 61.9 | 152 | 2.13 / 2.63 / 2.63 | 100.3 | 42.1 / 42.1 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.70 | - | met | 17.5 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 16.0 / 17.0 / 17.0 | 128 | 0.61 / 0.65 / 0.65 | 35.5 | 2.0 / 2.1 | 16.67 | 60.0 | 16.70 / 16.80 / 17.00 | 0 of 599 | 0.10 / 0.20 / 1.60 | - | met | 62.5 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 63.6 / 71.7 / 71.7 | 158 | 2.38 / 3.39 / 3.39 | 225.4 | 15.8 / 15.8 | 16.67 | 60.0 | 16.70 / 16.80 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.40 | - | met | 15.7 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgpu paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgpu groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgpu brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5
- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | 1904x757 | 5.77 | 36.2 / 41.5 / 42.6 | 1.6 / 9.2 / 9.7 | 158 | 37.8 |
| webgl2 | 1904x757 | 5.77 | 38.6 / 42.9 / 43.9 | 0.3 / 0.4 / 1.4 | 150 | 38.9 |
