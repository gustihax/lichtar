#!/usr/bin/env python3
"""Створює набір для перевірки Ліхтаря без справжніх патчів: тестову «гру», архіви патчів і каталоги.

  python tools/make_test_kit.py --out test-kit --big-mb 40

Далі: python tools/test_server.py test-kit/server  і кроки з TESTING.md.
"""
import argparse, datetime, hashlib, json, os, random, shutil, sys, zipfile
from pathlib import Path

MONTHS = ["січня","лютого","березня","квітня","травня","червня","липня","серпня","вересня","жовтня","листопада","грудня"]


def sha(p):
    h = hashlib.sha256()
    with open(p, 'rb') as f:
        for c in iter(lambda: f.read(1 << 20), b''):
            h.update(c)
    return h.hexdigest()


def size_txt(p):
    n = Path(p).stat().st_size
    mb = n / 1048576
    return f"{mb:.1f}".replace('.', ',') + ' МБ' if mb >= 1 else f"{max(1, round(n / 1024))} КБ"


def make_zip(path, files):
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as z:
        for name, data in files.items():
            z.writestr(name, data)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--out', default='test-kit')
    ap.add_argument('--big-mb', type=int, default=40, help='розмір великого патча для перевірки скасування й докачування')
    ap.add_argument('--port', type=int, default=8099)
    a = ap.parse_args()
    out = Path(a.out)
    game, srv = out / 'TestGame', out / 'server'
    for d in (game / 'data', srv):
        d.mkdir(parents=True, exist_ok=True)

    # «гра»: на Windows exe це копія Блокнота, щоб кнопка «Грати» щось запускала
    exe = game / 'TestGame.exe'
    notepad = Path(os.environ.get('SystemRoot', r'C:\Windows')) / 'System32' / 'notepad.exe'
    if os.name == 'nt' and notepad.is_file():
        shutil.copy(notepad, exe)
    else:
        exe.write_bytes(b'MZ' + b'\0' * 126)
    (game / 'data' / 'lang.txt').write_text('ORIGINAL\n', encoding='utf-8')
    (game / 'data' / 'ui.txt').write_text('ORIGINAL UI\n', encoding='utf-8')

    today = datetime.date.today()
    date = f"{today.day} {MONTHS[today.month - 1]}"
    zips = {
        '1.0': {'data/lang.txt': 'UKRAINIAN v1.0\n', 'data/ui.txt': 'UKRAINIAN UI v1.0\n', 'data/extra.txt': 'NEW FILE\n'},
        '1.1': {'data/lang.txt': 'UKRAINIAN v1.1\n', 'data/ui.txt': 'UKRAINIAN UI v1.1\n', 'data/extra.txt': 'NEW FILE\n'},
        '2.0': {'data/lang.txt': 'UKRAINIAN v2.0\n', 'data/big.bin': random.Random(7).randbytes(a.big_mb * 1048576)},
    }
    for v, files in zips.items():
        make_zip(srv / f'test-game-{v}.zip', files)

    def catalog(v, logs):
        z = srv / f'test-game-{v}.zip'
        return {
            'news': [{'d': date, 'ts': today.isoformat(), 'tag': 'Ліхтар', 't': 'Тестовий каталог підключено', 'x': 'Якщо ви це бачите, адреса каталогу вказана правильно.'}],
            'games': [{'id': 'test-game', 'name': 'Тестова гра', 'studio': 'Ліхтар', 'hue': 200, 'steam_appid': None,
                       'exe': ['TestGame.exe'], 'folders': ['TestGame'], 'version': v, 'size': size_txt(z),
                       'url': f'http://127.0.0.1:{a.port}/{z.name}', 'sha256': sha(z), 'log': logs}]}

    logs10 = [{'v': '1.0', 'd': date, 'i': [['', 'Перший тестовий патч']]}]
    logs11 = [{'v': '1.1', 'd': date, 'i': [['', 'Оновлено мовний файл']]}] + logs10
    logs20 = [{'v': '2.0', 'd': date, 'i': [['', 'Великий патч для перевірки скасування']]}] + logs11
    for name, cat in (('catalog.json', catalog('1.0', logs10)), ('catalog-update.json', catalog('1.1', logs11)), ('catalog-big.json', catalog('2.0', logs20))):
        (srv / name).write_text(json.dumps(cat, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

    print(f'Готово: {out.resolve()}')
    print(' Тестова гра :', game.resolve())
    print(' Сервер      :', srv.resolve(), '(catalog.json = версія 1.0, catalog-update.json = 1.1, catalog-big.json = 2.0 з великим файлом)')
    print(f' Запуск      : python tools/test_server.py {srv}  (додайте --kbps 300 для повільного завантаження)')
    print(f' Адреса в Ліхтарі: http://127.0.0.1:{a.port}/catalog.json')


if __name__ == '__main__':
    main()
