#!/usr/bin/env python3
"""Переносить catalog.json у вбудований резервний каталог в index.html (для першого запуску без мережі).

  python tools/sync_bundled.py [catalog.json] [src/index.html]

Запускайте перед збіркою програми, щоб вбудований каталог збігався з опублікованим.
"""
import json, re, sys
from pathlib import Path

cat_p = Path(sys.argv[1] if len(sys.argv) > 1 else 'catalog.json')
html_p = Path(sys.argv[2] if len(sys.argv) > 2 else 'src/index.html')
cat = json.loads(cat_p.read_text(encoding='utf-8'))
block = '/*BUNDLED:start*/\nconst BUNDLED = ' + json.dumps(cat, ensure_ascii=False, indent=2).replace('</', '<\\/') + ';\n/*BUNDLED:end*/'
s = html_p.read_text(encoding='utf-8')
new, n = re.subn(r'/\*BUNDLED:start\*/.*?/\*BUNDLED:end\*/', lambda m: block, s, count=1, flags=re.S)
if n != 1:
    sys.exit('У файлі немає маркерів /*BUNDLED:start*/ ... /*BUNDLED:end*/')
html_p.write_text(new, encoding='utf-8')
print(f'Вбудований каталог оновлено: ігор {len(cat.get("games", []))}, новин {len(cat.get("news", []))}')
