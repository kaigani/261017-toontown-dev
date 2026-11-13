import importlib.util,json,math
from pathlib import Path
from PIL import Image
r=Path.cwd();e=r/'reviews/evidence/M3-C3.4-r1';s=importlib.util.spec_from_file_location('n',r/'tests/adversarial/c34-r1-numeric.py');n=importlib.util.module_from_spec(s);s.loader.exec_module(n)
result=[]
for f in [1,12,24]:
 im=Image.open(e/f'local-a/frames/variant-{f:02d}.png').convert('RGB');total=sx=sy=0
 for y in range(im.height):
  for x in range(im.width):
   w=255-im.getpixel((x,y))[1];total+=w;sx+=w*(2*x+1);sy+=w*(2*y+1)
 # positive camera roll: world is rotated clockwise; parent CCW 30 minus camera CCW 15.
 a=math.radians(30-15);py=.8*n.position(f);ex=960-160*py*math.sin(a);ey=540-160*py*math.cos(a)
 error=max(abs(sx/(2*total)-ex),abs(sy/(2*total)-ey));assert error<.001
 result.append({'frame':f,'errorPx':error})
(e/'independent-variant.json').write_text(json.dumps(result,indent=2));print(result)
