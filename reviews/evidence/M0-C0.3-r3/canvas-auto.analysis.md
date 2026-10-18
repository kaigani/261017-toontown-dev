## Canvas spike log (version 2)

- source: auto; userAgent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36 Edg/155.0.0.0
- host: windows x86_64, Microsoft Windows [Version 10.0.26200.9711], webview 155.0.4283.62, CPUs 32; DPR 1, viewport {'height': 989, 'width': 1920}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | nvidia lovelace | 1904x757 | 8.70 | 70.4 / 81.7 / 81.7 | 123 | 3.67 / 5.07 / 5.07 | 130.5 | 27.7 / 99.0 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.20 | 8.30 / 10.70 / 12.50 | met | 14.2 |
| webgpu | groups1k | nvidia lovelace | 1904x757 | 2.02 | 16.3 / 18.7 / 18.7 | 122 | 0.95 / 1.00 / 1.00 | 32.0 | 3.7 / 13.3 | 16.67 | 60.0 | 16.70 / 16.90 / 17.00 | 0 of 600 | 0.10 / 0.20 / 0.20 | 3.30 / 4.80 / 6.40 | met | 61.3 |
| webgpu | brush10k | nvidia lovelace | 1904x757 | 10.09 | 71.3 / 88.7 / 88.7 | 138 | 5.03 / 5.37 / 5.37 | 255.4 | 1.6 / 20.5 | 16.67 | 60.0 | 16.70 / 16.90 / 17.10 | 0 of 599 | 0.10 / 0.20 / 0.30 | 4.90 / 6.50 / 7.90 | met | 14.0 |
| webgl2 | paths100k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 8.70 | 60.5 / 74.9 / 74.9 | 144 | 3.39 / 4.63 / 4.63 | 100.4 | 42.6 / 42.6 | 16.67 | 60.0 | 16.70 / 16.90 / 17.30 | 0 of 600 | 0.10 / 0.20 / 0.60 | - | met | 16.5 |
| webgl2 | groups1k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 2.02 | 13.2 / 17.7 / 17.7 | 145 | 0.78 / 0.96 / 0.96 | 37.1 | 2.2 / 2.2 | 16.67 | 60.0 | 16.70 / 16.90 / 17.20 | 0 of 600 | 0.10 / 0.20 / 1.60 | - | met | 75.8 |
| webgl2 | brush10k | ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11) | 1904x757 | 10.09 | 72.3 / 74.6 / 74.6 | 143 | 4.16 / 5.55 / 5.55 | 241.2 | 14.5 / 14.6 | 16.67 | 60.0 | 16.70 / 16.90 / 17.30 | 0 of 600 | 0.10 / 0.20 / 0.40 | - | met | 13.8 |

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
| webgpu | 1904x757 | 5.77 | 42.5 / 44.4 / 46.0 | 1.6 / 10.0 / 10.9 | 142 | 44.1 |
| webgl2 | 1904x757 | 5.77 | 34.7 / 45.0 / 46.6 | 0.4 / 0.4 / 1.6 | 150 | 35.1 |
