## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 51.7 / 57.5 / 57.5 | 166 | 2.69 / 3.22 / 3.22 | 96.5 | 22.9 / 70.4 | 16.67 | 60.0 | 16.70 / 16.80 / 17.10 | 0 of 599 | 0.10 / 0.20 / 0.50 | 8.10 / 11.50 / 12.90 | met | 19.3 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 12.7 / 14.3 / 14.3 | 157 | 0.63 / 0.75 / 0.75 | 32.0 | 3.5 / 13.4 | 16.67 | 60.0 | 16.70 / 16.80 / 17.00 | 0 of 599 | 0.20 / 0.30 / 0.80 | 4.40 / 5.40 / 7.20 | met | 78.7 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 70.9 / 87.8 / 87.8 | 145 | 4.74 / 5.28 / 5.28 | 256.1 | 1.6 / 19.6 | 16.67 | 60.0 | 16.70 / 16.80 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.50 | 5.30 / 7.40 / 9.30 | met | 14.1 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 59.7 / 68.8 / 68.8 | 145 | 2.77 / 4.15 / 4.15 | 99.7 | 42.4 / 42.4 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.60 | - | met | 16.8 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 13.2 / 18.1 / 18.1 | 145 | 0.73 / 1.10 / 1.10 | 16.4 | 2.9 / 2.9 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 599 | 0.10 / 0.20 / 1.00 | - | met | 75.8 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 66.5 / 74.4 / 74.4 | 148 | 3.87 / 5.46 / 5.46 | 234.4 | 14.8 / 14.8 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 599 | 0.10 / 0.20 / 0.40 | - | met | 15.0 |

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
| webgpu | 1904x757 | 5.77 | 36.7 / 44.3 / 44.5 | 1.5 / 9.4 / 9.8 | 152 | 38.2 |
| webgl2 | 1904x757 | 5.77 | 38.1 / 44.0 / 45.3 | 0.4 / 0.7 / 1.5 | 150 | 38.5 |
