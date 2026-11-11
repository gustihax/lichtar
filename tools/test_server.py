#!/usr/bin/env python3
"""Локальний сервер для перевірки Ліхтаря. Підтримує докачування (Range), може гальмувати швидкість.

  python tools/test_server.py test-kit/server --port 8099 --kbps 300

--kbps 0 означає без обмеження. У журналі видно запити й те, чи клієнт просив продовжити з певного байта.
"""
import argparse, re, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote


class Handler(BaseHTTPRequestHandler):
    root = Path('.')
    kbps = 0

    def log_message(self, *a):
        pass

    def note(self, status, start):
        rng = self.headers.get('Range', '-')
        print(f"{time.strftime('%H:%M:%S')}  {self.path}  Range: {rng}  ->  {status}" + (f" (з байта {start})" if status == 206 else ''), flush=True)

    def do_GET(self):
        p = (self.root / unquote(self.path.split('?')[0]).lstrip('/')).resolve()
        if not str(p).startswith(str(self.root)) or not p.is_file():
            self.send_error(404)
            self.note(404, 0)
            return
        size = p.stat().st_size
        start, end, status = 0, size - 1, 200
        m = re.match(r'bytes=(\d+)-(\d*)$', self.headers.get('Range', ''))
        if m:
            start = int(m[1])
            end = int(m[2]) if m[2] else size - 1
            if start >= size:
                self.send_response(416)
                self.send_header('Content-Range', f'bytes */{size}')
                self.send_header('Content-Length', '0')
                self.end_headers()
                self.note(416, start)
                return
            status = 206
        self.send_response(status)
        self.send_header('Accept-Ranges', 'bytes')
        self.send_header('Content-Length', str(end - start + 1))
        if status == 206:
            self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
        self.send_header('Content-Type', 'application/json; charset=utf-8' if p.suffix == '.json' else 'application/octet-stream')
        self.end_headers()
        self.note(status, start)
        with p.open('rb') as f:
            f.seek(start)
            left = end - start + 1
            while left > 0:
                data = f.read(min(16384, left))
                try:
                    self.wfile.write(data)
                except (BrokenPipeError, ConnectionResetError):
                    print('  клієнт обірвав з’єднання (скасування або помилка)', flush=True)
                    return
                left -= len(data)
                if self.kbps:
                    time.sleep(len(data) / (self.kbps * 1024))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('folder')
    ap.add_argument('--port', type=int, default=8099)
    ap.add_argument('--kbps', type=int, default=0)
    a = ap.parse_args()
    Handler.root = Path(a.folder).resolve()
    Handler.kbps = a.kbps
    if not Handler.root.is_dir():
        sys.exit(f'Немає теки {a.folder}')
    print(f'Сервер: http://127.0.0.1:{a.port}/  (тека {Handler.root}, швидкість {a.kbps or "без обмеження"} КБ/с). Зупинка: Ctrl+C')
    ThreadingHTTPServer(('127.0.0.1', a.port), Handler).serve_forever()


if __name__ == '__main__':
    main()
