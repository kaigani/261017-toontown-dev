from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'
s=p.read_text()
needle="expected={'failed_sequence_publication_leaves_no_frames'"
insert="ci_oracles=[]\nfor platform in ['Windows','macOS']:\n source=SRC/'artifacts'/('history-oracles-'+platform)\n if source.exists():\n  shutil.copytree(source,OUT/'ci-artifacts'/('history-oracles-'+platform),dirs_exist_ok=True)\n  for log in source.glob('*.log'):\n   text=read(log)\n   results=re.findall(r'test result: (ok|FAILED)\\. (\\d+) passed; (\\d+) failed;',text)\n   if results:\n    assert all(state=='ok' and failed=='0' for state,passed,failed in results),str(log)\n    ci_oracles.append(dict(platform=platform,file=log.name,summaries=results,sha256=sha(log.read_bytes())))\n"
s=s.replace(needle,insert+needle)
s=s.replace("nativeExits=exits,expectedFailingTests=", "nativeExits=exits,ciReleaseOracleSummaries=ci_oracles,expectedFailingTests=")
p.write_text(s)
