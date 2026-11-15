import subprocess
r = subprocess.run(["git","diff","--exit-code","f357ac4","0ca7f922","--","crates/*/tests/c42b_r*_review.rs","tests/adversarial/c42b-r*","desktop/src/test/c42b-r*","reviews/M4-C4.2b-r[12]*.md",":!reviews/M4-C4.2b-r*-response.md"], capture_output=True, text=True)
assert r.returncode == 0, r.stdout
