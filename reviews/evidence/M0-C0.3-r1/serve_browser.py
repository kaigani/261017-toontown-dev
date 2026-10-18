from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
class Handler(SimpleHTTPRequestHandler):
    def __init__(self,*args,**kwargs): super().__init__(*args,directory=str(ROOT/'target/review-browser'),**kwargs)
    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin','*')
        super().end_headers()
    def do_POST(self):
        if self.path!='/save': self.send_error(404); return
        data=self.rfile.read(int(self.headers['Content-Length']))
        (ROOT/'reviews/evidence/M0-C0.3-r1/browser-headroom.json').write_bytes(data)
        self.send_response(200); self.end_headers(); self.wfile.write(b'ok')
ThreadingHTTPServer(('127.0.0.1',1439),Handler).serve_forever()
