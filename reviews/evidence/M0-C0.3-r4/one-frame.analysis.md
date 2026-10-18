## Canvas spike log (version 2)

- source: interactive; userAgent: Mozilla/5.0 (win32) AppleWebKit/537.36 (KHTML, like Gecko) jsdom/30.1.3
- host: windows x86_64, review, webview jsdom, CPUs 1; DPR 1, viewport {'width': 1024, 'height': 768}
- WebGPU API present: False; forced renderer: webgl2
- parameters: {'scenes': ['paths100k', 'groups1k', 'brush10k'], 'seed': 1, 'transfers': 10, 'idleMs': 1000, 'panZoomMs': 10000, 'pixelTransfers': 20, 'flattenTolerance': 0.5}

### Runs (p50 / p95 / max)

| back end | scene | GPU | canvas px | MB / transfer | transfer ms | MB/s | encode ms | decode ms | upload ms (cpu / complete) | refresh ms | fps | interval ms | long frames | CPU frame ms | GPU frame ms | 60 fps target | IPC-per-frame ceiling fps |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| webgl2 | paths100k | review clock stub | 1x1 | 8.70 | 0.0 / 0.0 / 0.0 | - | 0.00 / 0.00 / 0.00 | 96.5 | 0.0 / 0.0 | 10000.00 | insufficient samples (n=1) | - | 0 of 0 | 0.00 / 0.00 / 0.00 | - | not estimated: insufficient samples (n=1) (run complete) | - |
| webgl2 | groups1k | review clock stub | 1x1 | 2.02 | 0.0 / 0.0 / 0.0 | - | 0.00 / 0.00 / 0.00 | 32.0 | 0.0 / 0.0 | 10000.00 | insufficient samples (n=1) | - | 0 of 0 | 0.00 / 0.00 / 0.00 | - | not estimated: insufficient samples (n=1) (run complete) | - |
| webgl2 | brush10k | review clock stub | 1x1 | 10.09 | 0.0 / 0.0 / 0.0 | - | 0.00 / 0.00 / 0.00 | 256.1 | 0.0 / 0.0 | 10000.00 | insufficient samples (n=1) | - | 0 of 0 | 0.00 / 0.00 / 0.00 | - | not estimated: insufficient samples (n=1) (run complete) | - |

Decode details (segments = stroke instances; triangles = fill triangles):

- webgl2 paths100k: paths 100000, cubics 200000, segments 2832713, fill vertices 0, triangles 0, GPU bytes 90.6 MB, tolerance 0.5
- webgl2 groups1k: paths 20000, cubics 50000, segments 101961, fill vertices 175915, triangles 145915, GPU bytes 7.8 MB, tolerance 0.5
- webgl2 brush10k: paths 10000, cubics 0, segments 0, fill vertices 1088402, triangles 1062652, GPU bytes 30.2 MB, tolerance 0.5

### Pixel-frame transfer (host-raster fallback, transfer + texture upload only)

| back end | size | MB / frame | transfer ms | upload ms | MB/s | transfer + upload p50 ms |
|---|---|---|---|---|---|---|
| webgl2 | 1x1 | 0.00 | 0.0 / 0.0 / 0.0 | 0.0 / 0.0 / 0.0 | - | 0.0 |
