"""Convert PowerShell UTF-16 redirected text to UTF-8 for reviewable git diffs."""
from pathlib import Path

for path in Path(__file__).resolve().parent.iterdir():
    if path.suffix not in {'.log', '.txt', '.json', '.patch'}:
        continue
    data = path.read_bytes()
    if data.startswith((b'\xff\xfe', b'\xfe\xff')):
        path.write_text(data.decode('utf-16'), encoding='utf-8', newline='\n')
