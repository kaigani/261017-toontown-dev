import subprocess,time,json
from pathlib import Path
R=Path(__file__).resolve().parents[3]
O=Path(__file__).resolve().parent
python=R/'.venv/Scripts/python.exe'
app=R/'target/release/toontown.exe'
head=R/'target/release/toontown-headless.exe'
modes=['labels','fixed-webgl2','fixed-webgpu','gui','exact-tools','c2','d2','d2-exact-attached','d2-exact-headless','d3-webgl2','d3-webgpu','alternate','named','byte','regression']
rows=[]
for mode in modes:
    t=time.monotonic()
    with (O/(mode+'-run.txt')).open('w',encoding='utf-8',newline='\n') as f:
        result=subprocess.run([str(python),str(R/'tests/adversarial/c42b-r4-live.py'),str(app),mode,str(head)],stdout=f,stderr=subprocess.STDOUT)
    tasks=subprocess.check_output(['tasklist','/FI','IMAGENAME eq toontown.exe'],text=True)
    rows.append(dict(mode=mode,code=result.returncode,seconds=time.monotonic()-t,processes=tasks.strip()))
    (O/'live-results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
    print(rows[-1],flush=True)
(O/'live.exit').write_text('done',encoding='utf-8')
