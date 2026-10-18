## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)
- host: macos aarch64, macOS 26.6.2 (25G83), webview 21624.5.1.11.3, CPUs 3; DPR 1, viewport {'height': 645, 'width': 1024}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgpu | groups1k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgpu | brush10k | **FAILED**: webgpu is not available in this webview | | | | | | | | | | | | | | | |
| webgl2 | paths100k | Apple GPU | 1008x417 | 8.70 | 18.0 / 166.0 / 166.0 | 188 | 2.53 / 24.25 / 24.25 | 416.0 | 64.0 / 64.0 | 16.64 | 22.1 | 44.00 / 64.00 / 166.00 | 218 of 221 | 0.00 / 1.00 / 1.00 | - | missed | 55.6 |
| webgl2 | groups1k | Apple GPU | 1008x417 | 2.02 | 3.0 / 13.0 / 13.0 | 547 | 0.39 / 0.61 / 0.61 | 60.0 | 1.0 / 1.0 | 16.51 | 60.0 | 17.00 / 19.00 / 30.00 | 4 of 600 | 0.00 / 1.00 / 1.00 | - | met | 333.3 |
| webgl2 | brush10k | Apple GPU | 1008x417 | 10.09 | 10.0 / 35.0 / 35.0 | 820 | 1.45 / 6.92 / 6.92 | 493.0 | 7.0 / 7.0 | 16.80 | 60.0 | 17.00 / 18.00 / 20.00 | 0 of 600 | 0.00 / 0.00 / 1.00 | - | met | 100.0 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu is not available | | | | | |
| webgl2 | 1008x417 | 1.68 | 2.0 / 3.0 / 12.0 | 0.0 / 1.0 / 1.0 | 611 | 2.0 |
