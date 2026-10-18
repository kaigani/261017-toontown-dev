## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 54.3 / 56.6 / 56.6 | 162 | 1.92 / 2.41 / 2.41 | 87.2 | 22.4 / 67.1 | 16.95 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 599 | 0.10 / 0.20 / 0.30 | 8.40 / 11.20 / 13.40 | met | 18.4 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 12.9 / 16.3 / 16.3 | 148 | 0.52 / 0.62 / 0.62 | 29.9 | 3.5 / 13.5 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.20 / 0.30 / 0.50 | 4.50 / 5.40 / 6.60 | met | 77.5 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 72.2 / 88.8 / 88.8 | 141 | 2.52 / 3.13 / 3.13 | 239.0 | 1.7 / 21.0 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 599 | 0.10 / 0.20 / 0.60 | 5.10 / 7.40 / 10.00 | met | 13.9 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 59.6 / 64.4 / 64.4 | 147 | 2.19 / 3.23 / 3.23 | 88.0 | 43.3 / 43.3 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.40 | - | met | 16.8 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 12.5 / 17.1 / 17.1 | 148 | 0.44 / 0.62 / 0.62 | 37.0 | 2.6 / 2.6 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.70 | - | met | 80.0 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 60.5 / 71.2 / 71.2 | 158 | 2.08 / 3.10 / 3.10 | 250.8 | 14.7 / 14.7 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.60 | - | met | 16.5 |

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
| webgpu | 1904x757 | 5.77 | 34.1 / 40.8 / 41.3 | 1.7 / 9.5 / 10.5 | 162 | 35.8 |
| webgl2 | 1904x757 | 5.77 | 43.5 / 47.7 / 48.6 | 0.4 / 0.7 / 1.8 | 138 | 43.9 |
