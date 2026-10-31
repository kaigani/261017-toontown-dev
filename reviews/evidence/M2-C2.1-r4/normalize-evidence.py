"""Normalize display logs only; compressed CI logs preserve original bytes."""
from pathlib import Path
import gzip
HERE=Path(__file__).resolve().parent
failed=HERE/'ci-failed.log'
raw=HERE/'ci-failed.log.gz'
if not raw.exists(): raw.write_bytes(gzip.compress(failed.read_bytes(),mtime=0))
for pattern in ['*.log','*.txt']:
 for p in HERE.glob(pattern):
  s=p.read_text(encoding='utf8')
  s='\n'.join(line.rstrip() for line in s.splitlines()).rstrip()+'\n'
  p.write_text(s,encoding='utf8',newline='\n')
