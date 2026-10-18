"""Retain reproducible upstream citations, scoped grep, and exact diff receipt."""
import datetime
import gzip
import hashlib
import json
import pathlib
import re
import subprocess
import urllib.request

OUT = pathlib.Path(__file__).resolve().parent
ROOT = OUT.parents[2]
sources = {
    'ffv1enc': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/libavcodec/ffv1enc.c',
                r'Lesser|version 2.1|lossless|AV_CODEC_ID_FFV1|AV_PIX_FMT_BGRA'),
    'configure': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/configure',
                  r'ffv1_encoder|disable-everything|disable-autodetect|enable-encoder'),
    'movenc': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/libavformat/movenc.c',
               r'BITEXACT|creation_time|faststart|video_track_timescale|write_prft'),
    'mux': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/libavformat/mux.c',
            r'BITEXACT|encoder|LIBAVFORMAT_IDENT'),
    'formats-doc': ('https://ffmpeg.org/ffmpeg-formats.html', r'bitexact|faststart|time-independent'),
    'ffmpeg-doc': ('https://ffmpeg.org/ffmpeg.html', r'negative file index|map_metadata|delete metadata'),
}
receipts = {}
for name, (url, pattern) in sources.items():
    data = urllib.request.urlopen(url, timeout=30).read()
    receipts[name] = dict(url=url, retrievedUtc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                          sha256=hashlib.sha256(data).hexdigest(), bytes=len(data),
                          excerpts=[dict(line=i, text=line[:700]) for i, line in
                                    enumerate(data.decode('utf-8').splitlines(), 1) if re.search(pattern, line)])
(OUT/'primary-source-receipts.json').write_text(json.dumps(receipts, indent=2)+'\n', encoding='utf-8')
diff = subprocess.check_output(['git','diff','48a809b..a89ca1b'], cwd=ROOT)
(OUT/'reviewed.diff.gz').write_bytes(gzip.compress(diff, mtime=0))
print('Fetched', len(receipts), 'primary sources; diff bytes', len(diff))
