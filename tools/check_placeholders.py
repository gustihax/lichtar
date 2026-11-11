#!/usr/bin/env python3
"""Показує, що ще треба замінити перед випуском: пошта, нік, адреса каталогу, ключ оновлень.

  python tools/check_placeholders.py

Запускайте з кореня репозиторію програми. Код виходу 1, якщо щось не заповнено.
"""
import re, sys
from pathlib import Path

CHECKS = [
    ('src/index.html', r"email:\s*'your@email\.com'", 'Впишіть свою пошту: SUPPORT.email угорі скрипта'),
    ('src/index.html', r"github\.com/USER/", 'Впишіть свій нік у SUPPORT.issues (адреса репозиторію програми)'),
    ('src/index.html', r"const CATALOG_URL = '';", "Впишіть адресу каталогу: CATALOG_URL = 'https://raw.githubusercontent.com/НІК/lichtar-catalog/main/catalog.json'"),
    ('src-tauri/tauri.release.conf.json', r"ВСТАВТЕ", 'Вставте публічний ключ у pubkey (команда signer generate, див. START_HERE.md)'),
    ('src-tauri/tauri.release.conf.json', r"ВАШ_НІК", 'Впишіть свій нік в endpoints (адреса latest.json)'),
]


def main():
    root = Path('.')
    bad = 0
    for rel, pat, msg in CHECKS:
        p = root / rel
        if not p.is_file():
            print(f'ПОМИЛКА: немає файлу {rel} (запускайте з кореня репозиторію програми)')
            bad += 1
            continue
        for n, line in enumerate(p.read_text(encoding='utf-8').splitlines(), 1):
            if re.search(pat, line):
                print(f'Не заповнено: {rel}:{n}\n    {msg}')
                bad += 1
                break
    if re.search(r"const STATS_URL = '';", (root / 'src/index.html').read_text(encoding='utf-8')) if (root / 'src/index.html').is_file() else False:
        print('Інформація: STATS_URL порожній, тому статистика встановлень вимкнена (так і задумано)')
    print(f'\nНе заповнено місць: {bad}.' + (' Можна випускати.' if not bad else ''))
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
