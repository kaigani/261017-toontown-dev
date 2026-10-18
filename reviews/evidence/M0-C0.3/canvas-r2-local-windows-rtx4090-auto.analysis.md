## Canvas spike log (version 2)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 52.5 / 58.2 / 58.2 | 164 | 3.27 / 3.56 / 3.56 | 96.6 | 21.9 / 68.7 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.40 | 8.70 / 11.70 / 13.30 | met | 19.0 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 12.6 / 15.1 / 15.1 | 157 | 0.72 / 0.77 / 0.77 | 32.2 | 3.6 / 13.9 | 16.67 | 60.0 | 16.70 / 16.80 / 17.10 | 0 of 600 | 0.20 / 0.30 / 0.70 | 4.50 / 5.50 / 8.20 | met | 79.4 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 70.1 / 86.9 / 86.9 | 142 | 3.87 / 5.43 / 5.43 | 255.3 | 1.6 / 20.5 | 16.67 | 60.0 | 16.70 / 16.80 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.30 | 5.30 / 7.40 / 9.30 | met | 14.3 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 57.5 / 64.7 / 64.7 | 147 | 2.69 / 4.47 / 4.47 | 96.4 | 42.8 / 42.8 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 600 | 0.10 / 0.20 / 0.60 | - | met | 17.4 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 13.7 / 19.1 / 19.1 | 138 | 0.77 / 1.10 / 1.10 | 39.3 | 2.3 / 2.3 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 599 | 0.10 / 0.20 / 1.60 | - | met | 73.0 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 68.6 / 73.9 / 73.9 | 148 | 3.69 / 5.36 / 5.36 | 241.5 | 14.8 / 14.8 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 599 | 0.10 / 0.20 / 0.30 | - | met | 14.6 |

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
| webgpu | 1904x757 | 5.77 | 35.7 / 41.2 / 44.3 | 1.6 / 9.9 / 10.0 | 158 | 37.3 |
| webgl2 | 1904x757 | 5.77 | 38.2 / 43.3 / 44.8 | 0.4 / 0.4 / 1.5 | 151 | 38.6 |
