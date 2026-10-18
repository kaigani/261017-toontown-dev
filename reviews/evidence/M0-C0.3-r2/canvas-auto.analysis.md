## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 51.4 / 55.4 / 55.4 | 168 | 2.68 / 2.92 / 2.92 | 97.2 | 21.7 / 69.6 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 599 | 0.00 / 0.10 / 0.20 | 7.60 / 10.60 / 13.10 | met | 19.5 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 13.1 / 18.2 / 18.2 | 147 | 0.63 / 0.88 / 0.88 | 32.9 | 3.2 / 13.3 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.20 | 3.50 / 4.60 / 6.10 | met | 76.3 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 70.5 / 86.0 / 86.0 | 144 | 4.04 / 5.05 / 5.05 | 264.0 | 1.8 / 19.8 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 599 | 0.00 / 0.10 / 0.20 | 4.40 / 5.80 / 7.60 | met | 14.2 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 61.7 / 63.5 / 63.5 | 141 | 3.78 / 4.35 / 4.35 | 95.5 | 41.3 / 41.3 | 16.67 | 60.0 | 16.70 / 17.00 / 17.00 | 0 of 599 | 0.00 / 0.10 / 0.20 | - | met | 16.2 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 13.6 / 17.5 / 17.5 | 139 | 0.65 / 0.98 / 0.98 | 37.3 | 2.3 / 2.4 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 599 | 0.00 / 0.10 / 0.40 | - | met | 73.5 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 70.6 / 72.3 / 72.3 | 149 | 3.96 / 5.06 / 5.06 | 243.5 | 14.3 / 14.4 | 16.66 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.00 / 0.10 / 0.20 | - | met | 14.2 |

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
| webgpu | 1904x757 | 5.77 | 42.1 / 43.0 / 43.7 | 1.6 / 10.2 / 10.5 | 137 | 43.7 |
| webgl2 | 1904x757 | 5.77 | 54.2 / 55.3 / 56.6 | 0.4 / 0.5 / 1.5 | 107 | 54.6 |
