## Canvas spike log (version 2)

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
| webgl2 | paths100k | Apple GPU | 1008x417 | 8.70 | 8.0 / 20.0 / 20.0 | 978 | 2.70 / 4.31 / 4.31 | 351.0 | 10.0 / 10.0 | 16.67 | 21.2 | 46.00 / 71.00 / 106.00 | 204 of 212 | 0.00 / 1.00 / 1.00 | - | missed | 125.0 |
| webgl2 | groups1k | Apple GPU | 1008x417 | 2.02 | 2.0 / 10.0 / 10.0 | 698 | 0.64 / 1.36 / 1.36 | 59.0 | 0.0 / 0.0 | 16.62 | 59.5 | 17.00 / 20.00 / 42.00 | 6 of 594 | 0.00 / 1.00 / 1.00 | - | met | 500.0 |
| webgl2 | brush10k | Apple GPU | 1008x417 | 10.09 | 7.0 / 20.0 / 20.0 | 1187 | 3.38 / 4.27 / 4.27 | 451.0 | 4.0 / 4.0 | 16.62 | 59.7 | 17.00 / 20.00 / 36.00 | 6 of 596 | 0.00 / 1.00 / 1.00 | - | met | 142.9 |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgpu | **FAILED**: webgpu unavailable: navigator.gpu.requestAdapter() returned null | | | | | |
| webgl2 | 1008x417 | 1.68 | 2.0 / 3.0 / 11.0 | 1.0 / 1.0 / 1.0 | 659 | 3.0 |
