"""Normalize PowerShell-redirection text to UTF-8 for review in Git."""
from pathlib import Path
root=Path(__file__).resolve().parent
for path in root.iterdir():
 if not path.is_file() or path.suffix not in {'.log','.txt','.patch','.json'}:
  continue
 data=path.read_bytes()
 text=data.decode('utf-16' if data.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig')
 path.write_text(text.replace('\r\n','\n'),encoding='utf-8',newline='\n')
