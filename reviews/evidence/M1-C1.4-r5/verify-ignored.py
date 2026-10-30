import pathlib,subprocess
R=pathlib.Path.cwd()
s=(R/'reviews/evidence/M1-C1.4-r4/register.py').read_text()
checks=s[s.index('cases=['):s.index("print(d['counts']")]
exec(checks)
print('Ignored test bodies: all five unchanged from introduction; inherited owner/deadline dispositions retained.')
