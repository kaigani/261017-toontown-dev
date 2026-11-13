from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'; s=p.read_text()
s=s.replace("path.parent.mkdir(parents=True,exist_ok=True); path.write_text(text.replace('\\r\\n','\\n'),encoding='utf-8',newline='\\n')", "path.parent.mkdir(parents=True,exist_ok=True)\n normalized='\\n'.join(line.rstrip() for line in text.splitlines()).rstrip()\n path.write_text(normalized+('\\n' if normalized else ''),encoding='utf-8',newline='\\n')")
needle="expected={'failed_sequence_publication_leaves_no_frames'"
insert="for log in (OUT/'ci-artifacts').rglob('*.log'):\n data=log.read_bytes(); target=raw/'ci-artifacts'/(log.relative_to(OUT/'ci-artifacts').as_posix()+'.gz'); target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes(gzip.compress(data,mtime=0)); write(log,read(log))\n"
s=s.replace(needle,insert+needle)
s=s.replace('human copies are UTF-8/LF','human copies are UTF-8/LF with trailing whitespace removed')
p.write_text(s)
