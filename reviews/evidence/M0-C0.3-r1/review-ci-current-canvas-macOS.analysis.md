## Canvas spike log (version 1)

- source: auto; userAgent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)
- host: macos aarch64, macOS 26.6.2 (25G83), webview 21624.5.1.11.3, CPUs 3; DPR 1, viewport {'height': 645, 'width': 1024}
- WebGPU API present: True; forced renderer: None
- parameters: {'flattenTolerance': 0.5, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgpu | paths100k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgpu | groups1k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgpu | brush10k | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | | | | | | | | | | | |
| webgl2 | paths100k | Apple GPU | 1008x417 | 8.70 | 15.0 / 22.0 / 22.0 | 649 | 1.80 / 3.50 / 3.50 | 341.0 | 35.0 / 35.0 | 16.66 | 22.0 | 42.00 / 66.00 / 98.00 | 216 of 220 | 0.00 / 1.00 / 1.00 | - | missed | 66.7 |
| webgl2 | groups1k | Apple GPU | 1008x417 | 2.02 | 4.0 / 26.0 / 26.0 | 321 | 0.40 / 2.53 / 2.53 | 109.0 | 3.0 / 3.0 | 16.93 | 59.8 | 17.00 / 19.00 / 38.00 | 1 of 598 | 0.00 / 0.00 / 1.00 | - | met | 250.0 |
| webgl2 | brush10k | Apple GPU | 1008x417 | 10.09 | 8.0 / 37.0 / 37.0 | 901 | 1.42 / 3.84 / 3.84 | 462.0 | 8.0 / 8.0 | 16.90 | 57.6 | 17.00 / 19.00 / 171.00 | 8 of 575 | 0.00 / 0.00 / 1.00 | - | met | 125.0 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | |
| webgl2 | 1008x417 | 1.68 | 3.0 / 6.0 / 15.0 | 0.0 / 1.0 / 1.0 | 448 | 3.0 |
