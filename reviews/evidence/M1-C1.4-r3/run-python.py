"""Run the complete discovered Python suite list with the exact-head CI executable and retain native exit status."""
import json
import os
import pathlib
import subprocess
import sys
HERE=pathlib.Path(__file__).resolve().parent
ROOT=HERE.parents[2]
env={**os.environ,'TT_REVIEW_BINARY':str(ROOT/'target/review-ci-r3/headless-Windows/toontown-headless.exe'),'PYTHONIOENCODING':'utf-8'}
with (HERE/'python-final.log').open('wb') as log:
    p=subprocess.run([sys.executable,'scripts/run-reviewer-python-suites.py'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
(HERE/'python-final-status.json').write_text(json.dumps({'exitCode':p.returncode,'expected':'13 original files pass; two r3 files fail new findings','binary':env['TT_REVIEW_BINARY']},indent=2)+'\n',encoding='utf-8')
print('Python suite runner native exit:',p.returncode)
