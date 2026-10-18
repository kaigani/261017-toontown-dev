"""Fetch primary upstream references; save small checkable receipts, not mutable links alone."""
import datetime
import hashlib
import json
from pathlib import Path
from urllib.request import urlopen, Request

sources = {
    'ffmpeg-configure-n8.0': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/configure',
        ['--disable-autodetect','--disable-encoders','--enable-encoder=','--enable-gpl','--enable-mediafoundation']),
    'ffmpeg-mf-n8.0': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/libavcodec/mfenc.c',
        ['MF_ENCODER(AUDIO, aac','MF_ENCODER(VIDEO, h264','Windows N editions']),
    'ffmpeg-at-n8.0': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/libavcodec/audiotoolboxenc.c',
        ['FFAT_ENC(aac']),
    'ffmpeg-lgpl': ('https://raw.githubusercontent.com/FFmpeg/FFmpeg/n8.0/COPYING.LGPLv2.1', []),
    'apple-prores': ('https://www.apple.com/final-cut-pro/docs/Apple_ProRes.pdf', []),
}
result = {}
for key,(url,markers) in sources.items():
    with urlopen(Request(url,headers={'User-Agent':'Toontown-C04-review'}),timeout=30) as r:
        data=r.read()
    lines=data.decode('utf-8',errors='replace').splitlines() if markers else []
    result[key]={'url':url,'retrievedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),
        'matching_lines':[{'line':i+1,'text':line} for i,line in enumerate(lines) if any(m in line for m in markers)]}
Path(__file__).with_name('primary-source-receipts.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print('primary-source receipts saved')
