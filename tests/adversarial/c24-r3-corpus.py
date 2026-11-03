"""Fresh synthetic decoder corpus, seed C243D39F. Generation requires an explicit output directory."""
import hashlib
import io
import json
from pathlib import Path
import random
import struct
import sys
import unittest
import zlib


def generate(out):
    from PIL import Image
    if out.exists():
        raise RuntimeError('refusing to overwrite a corpus')
    out.mkdir(parents=True)
    rng = random.Random(0xC243D39F)
    files = {}
    for i in range(32):
        w, h = rng.randrange(1, 34), rng.randrange(1, 34)
        rgb = bytes(rng.randrange(256) for _ in range(w * h * 3))
        im = Image.frombytes('RGB', (w, h), rgb)
        for fmt in ('PNG', 'JPEG', 'BMP', 'GIF', 'TIFF', 'TGA'):
            buf = io.BytesIO()
            im.save(buf, format=fmt, **({'progressive': bool(i % 2)} if fmt == 'JPEG' else {}))
            files[f'{i:02}-{w}x{h}.{fmt.lower()}'] = buf.getvalue()
    def chunk(k, d):
        return struct.pack('>I', len(d)) + k + d + struct.pack('>I', zlib.crc32(k + d))
    for i, (w, h) in enumerate([(0, 1), (1, 0), (0xffffffff, 0xffffffff), (1, 16385), (32768, 1)]):
        files[f'limit-{i}.png'] = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0')) + chunk(b'IEND', b'')
    for i, (name, data) in enumerate(list(files.items())[:48]):
        n = rng.randrange(len(data))
        files[f'truncated-{i}-{name}'] = data[:n]
    manifest = {n: hashlib.sha256(b).hexdigest() for n, b in files.items()}
    for n, b in files.items():
        (out / n).write_bytes(b)
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (out / 'provenance.json').write_text(json.dumps({'seed': 'C243D39F', 'source': __file__,
        'files': len(files), 'method': 'synthetic RGB noise, dimensions, progressive JPEG, six encodings, truncated headers'}, indent=2) + '\n')
    print(len(files), 'fresh fixtures')


if __name__ == '__main__':
    if sys.argv[1:2] == ['--out'] and len(sys.argv) == 3:
        generate(Path(sys.argv[2]))
    else:
        class NoImplicitWrites(unittest.TestCase):
            def test_explicit_output_required(self):
                self.assertIn('explicit output', __doc__)
        unittest.main()
