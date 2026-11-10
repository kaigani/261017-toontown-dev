from pathlib import Path
import tempfile
p=Path(tempfile.gettempdir())/'toontown-c33-r1/package-evidence.py'
s=p.read_text()
needle="for platform in ['Windows','macOS']:\n source=SRC/'artifacts'/('c33-'+platform)"
insert="review_goldens=[('transparent-32x18.png',32,18,'43b6c3ea64e44fb4b2195b086533047442b635f2cebeb6ecf52e21c1d826f238'),('solid-32x18.png',32,18,'187b989d25ae7ae51211c215bf156816e96854313178964ff70460245d432442'),('rectangle-0-0.05.png',120,68,'e6130113f9b2a264487cdcdd31a1bd2fcbd4d148653bba7e6078beabec42c1a0'),('rectangle-37-0.png',120,68,'58317fd335465043c65931a91360a0dc3a0e7af40a3755e29f54d17239db6dbb')]\nfor name,w,h,digest in review_goldens: assert sha((OUT/'goldens'/name).read_bytes())==digest\nwrite(OUT/'goldens/index.json',json.dumps(dict(reviewedHead='e61c2651feb83c7e03174bedd10fa2cf563c4ad8',source='crates/executor/tests/c33_r1_pixels.rs',images=[dict(name=n,width=w,height=h,sha256=d) for n,w,h,d in review_goldens]),indent=2)+'\\n')\n"
s=s.replace(needle,insert+needle,1)
p.write_text(s)
