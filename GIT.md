# Git і GitHub для Ліхтаря: повний гайд

Команди для PowerShell у VS Code (меню **Terminal → New Terminal**). Усе, що є командою, можна зробити й кнопками VS Code або на сайті GitHub. Усюди нижче `НІК` це ваш нікнейм на GitHub.

## 0. Що де лежить

Потрібні **два публічні репозиторії**:

```
lichtar/                         ← програма (код; інсталятор з'являється в Releases)
  package.json, package-lock.json            (створює create-tauri-app, npm install)
  icon.svg, GIT.md, TESTING.md
  src/index.html
  src-tauri/Cargo.toml, Cargo.lock, tauri.conf.json, tauri.release.conf.json
  src-tauri/capabilities/default.json, src-tauri/icons/ (npm run tauri icon)
  src-tauri/src/main.rs, lib.rs, installer.rs, download.rs
  tools/sync_bundled.py, check_placeholders.py, make_test_kit.py, test_server.py
  .github/workflows/release.yml

lichtar-catalog/                 ← каталог і патчі (патчі лежать у Releases)
  catalog.json, README.md
  tools/release_to_catalog.py, validate_catalog.py, inspect_game.py
  .github/workflows/catalog-on-release.yml, validate-catalog.yml
```

Чому два: адреса оновлення програми це «останній реліз» репозиторію, і патчі її б затьмарили. Чому публічні: з приватного репозиторію лончер не зможе завантажити каталог і патчі без входу.

Папка `.github` має бути **в корені** репозиторію, інакше Actions її не побачать.

## 1. Один раз: встановлення

```
winget install Git.Git
winget install GitHub.cli
```
Закрийте й знову відкрийте VS Code. Потім:
```
git config --global user.name "НІК"
git config --global user.email "НІК@users.noreply.github.com"
git config --global init.defaultBranch main
gh auth login
```
Для `gh auth login` оберіть: GitHub.com → HTTPS → Login with a web browser. Це заодно налаштовує вхід для `git push`. Якщо хочете лише VS Code: меню Accounts внизу ліворуч → Sign in with GitHub.

Адресу `НІК@users.noreply.github.com` використовуйте, щоб не світити справжню пошту. Увімкнути це можна на GitHub: Settings → Emails → Keep my email addresses private.

## 2. Репозиторій програми (`lichtar`)

Відкрийте в VS Code папку проєкту, у якій лежить `package.json`.

**Перед першим комітом** перевірте `.gitignore` у корені: у ньому мають бути `node_modules`, а в `src-tauri/.gitignore` рядок `/target`. Їх створює шаблон Tauri. Додайте ще два рядки на випадок помилки з ключем:
```
*.key
.tauri/
```

**Спосіб А. Кнопками VS Code**
1. Значок Source Control зліва (`Ctrl+Shift+G`) → **Initialize Repository**.
2. Біля «Changes» натисніть «+», щоб додати все.
3. У полі повідомлення напишіть `Перший коміт` → **Commit**.
4. Натисніть **Publish Branch** → оберіть **Publish to GitHub public repository**, назва `lichtar`. Готово.

**Спосіб Б. Командами**
```
git init
git add .
git status
git commit -m "Перший коміт"
git branch -M main
git remote add origin https://github.com/НІК/lichtar.git
git push -u origin main
```
Репозиторій перед цим створіть на github.com: **New repository**, назва `lichtar`, **Public**, **не** ставте галочки README, .gitignore, license. Після `git status` перегляньте список: там не має бути `node_modules` і `target`.

**Спосіб В. Одна команда (GitHub CLI)**
```
git init
git add .
git commit -m "Перший коміт"
gh repo create lichtar --public --source=. --remote=origin --push
```

## 3. Репозиторій каталогу (`lichtar-catalog`)

1. Створіть нову папку `lichtar-catalog` і складіть у неї файли за схемою з розділу 0. `catalog.json`, `RELEASING.md`, `tools/release_to_catalog.py` і `.github/workflows/catalog-on-release.yml` це файли з чату.
2. Відкрийте цю папку в VS Code й повторіть будь-який зі способів розділу 2, тільки з назвою `lichtar-catalog`.

У репозиторій програми файл `catalog-on-release.yml` **не кладіть**, туди йде лише `release.yml`.

## 4. Налаштування на GitHub (в обох репозиторіях)

1. **Settings → Actions → General → Workflow permissions → Read and write permissions → Save.** Без цього бот не зможе оновити каталог, а збірка програми не зможе створити реліз.
2. У репозиторії `lichtar`: **Settings → Secrets and variables → Actions → New repository secret**. Створіть два секрети:
   - `TAURI_SIGNING_PRIVATE_KEY`: увесь вміст файлу `C:\Users\ВАШ_КОРИСТУВАЧ\.tauri\lichtar.key` (відкрийте в Блокноті, скопіюйте все);
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: пароль, який ви задали при створенні ключа.

   Те саме командами: `Get-Content $env:USERPROFILE\.tauri\lichtar.key -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY`, потім `gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
3. Ліцензія: на сторінці репозиторію **Add file → Create new file**, ім'я `LICENSE`, праворуч з'явиться кнопка **Choose a license template**. Оберіть ліцензію, яка вам підходить (наприклад, MIT дозволяє будь-яке використання, GPL вимагає відкритого коду похідних).

## 5. Щоденна робота

Кнопками: Source Control → «+» біля змінених файлів → повідомлення → **Commit** → **Sync Changes**.

Командами:
```
git pull
git add .
git commit -m "Що зроблено"
git push
```

**Золоте правило для каталогу: спершу `git pull`.** Бот сам комітить у `lichtar-catalog` при кожному патчі, тож ваша локальна копія швидко відстає. Якщо `git push` каже `rejected`, виконайте:
```
git pull --rebase
git push
```
Корисне: `git status` (що змінилося), `git diff` (що саме), `git log --oneline` (історія).

## 6. Випуск нової версії програми

1. Підніміть `"version"` у `src-tauri/tauri.conf.json`, наприклад `0.1.1`. **Версія в конфігу має збігатися з тегом без літери v**, інакше встановлені програми не побачать оновлення.
2. Закомітьте й запушіть: `git add .`, `git commit -m "Версія 0.1.1"`, `git push`.
3. Створіть тег і відправте його (звичайний `git push` теги не відправляє):
   ```
   git tag v0.1.1
   git push origin v0.1.1
   ```
   У VS Code теж можна: `Ctrl+Shift+P` → **Git: Create Tag**, потім **Git: Push Tags**.
4. Відкрийте вкладку **Actions** репозиторію `lichtar`: збірка йде 10-15 хвилин. Коли позначка зелена, на сторінці **Releases** з'явиться `Lichtar_0.1.1_x64-setup.exe`. Це посилання ви даєте людям.

Перший раз спробуйте з тегом `v0.1.0-test`, потім видаліть тестовий реліз (розділ 7).

## 7. Випуск патча (репозиторій `lichtar-catalog`)

**На сайті**
1. Сторінка репозиторію → праворуч **Releases → Draft a new release**.
2. **Choose a tag** → введіть `blood-west-v1.4.3` → **Create new tag on publish**. Target: `main`.
3. Заголовок будь-який, наприклад `Blood West 1.4.3`.
4. В описі, що виправлено, по рядку з дефісом. Необов'язково: рядок `збірка: 1234567`.
5. Перетягніть zip-архів у блок «Attach binaries».
6. Для проби поставте галочку **Set as a pre-release** (каталог не зміниться). Для справжнього: зніміть її.
7. **Publish release**. За хвилину в `catalog.json` з'явиться новий запис, його видно у вкладці **Actions** і в історії комітів.

**Командою**
```
gh release create blood-west-v1.4.3 .\BloodWest_UA.zip --title "Blood West 1.4.3" --notes-file notes.txt
```
Додайте `--prerelease` для проби. Файл `notes.txt` це ваш список змін.

**Видалити тестовий реліз і тег**
```
gh release delete blood-west-v9.9 --cleanup-tag --yes
```
Або на сайті: відкрийте реліз → **Delete**, потім у вкладці Tags видаліть тег. Якщо тег створено локально:
```
git tag -d v0.1.0-test
git push origin --delete v0.1.0-test
```

**Скасувати поганий запис у каталозі.** Знайдіть коміт у `git log --oneline` і виконайте `git revert НОМЕР_КОМІТУ`, потім `git push`. Простіше: відкрийте `catalog.json` на GitHub, натисніть олівець, виправте, **Commit changes**.

## 8. Нова гра і новини вручну

Усе це правиться в `catalog.json` прямо на GitHub (олівець → **Commit changes**), без VS Code.

Нова гра: блок для масиву `games` є в `RELEASING.md`.

Новина: рядок у масив `news` (раніше я писав про окремий `news.json`, насправді новини лежать у цьому ж файлі):
```json
{ "d": "16 жовтня", "ts": "2026-10-16", "tag": "Ліхтар", "t": "Заголовок", "x": "Короткий опис", "u": "https://посилання" }
```
`d` це текст дати, `ts` це дата для сортування (рік-місяць-день). Необов'язково: `"img"` (посилання на картинку) і `"game"` (id гри, тоді з'явиться кнопка «До гри»). Свіжі новини ставте на початок.

## 9. Чого не можна комітити

- Файл ключа `lichtar.key` та пароль. Якщо ключ потрапив у репозиторій, **видалення не допоможе**: ключ уже в історії. Згенеруйте новий, замініть `pubkey` та секрети (програми, уже встановлені зі старим ключем, перестануть оновлюватись, тому бережіть ключ).
- `node_modules`, `src-tauri/target`, готові `.exe`: їх не коміть, інсталятор потрапляє в Releases сам.
- Патчі (zip) у репозиторій. Файл у репозиторії обмежений 100 МБ, а в Release до 2 ГБ. Патчі лише в Releases.
- Обов'язково комітити: `package-lock.json`, `src-tauri/Cargo.lock`, `src-tauri/icons`. Без першого `npm ci` на GitHub не спрацює.

## 10. Якщо щось пішло не так

| Помилка | Що робити |
|---|---|
| `fatal: not a git repository` | Ви не в папці проєкту. `cd` у папку, де є `.git`. |
| `src refspec main does not match any` | Ще немає жодного коміту (`git add .` і `git commit`), або гілка називається master: `git branch -M main`. |
| `remote origin already exists` | `git remote set-url origin https://github.com/НІК/lichtar.git` |
| `rejected ... fetch first` | `git pull --rebase`, потім `git push`. |
| `Authentication failed` / `Permission denied` | `gh auth login` або вхід через Accounts у VS Code. |
| Actions: `Resource not accessible by integration` | Розділ 4, п. 1: дозвіл Read and write. |
| Actions (програма): `npm ci` падає | Не закомічено `package-lock.json`. |
| Actions (програма): помилка про ключ підпису | Немає або неправильні секрети (розділ 4, п. 2). |
| Actions (каталог): `Гри «…» немає в catalog.json` | Додайте блок гри в `catalog.json` (див. `RELEASING.md`). |
| Actions (каталог): `Тег … не підходить` | Тег має вигляд `id-гри-v1.4.3`. |
| Actions (каталог): `У релізі немає zip-архіву` | Прикріпіть `.zip` до релізу. |
| `LF will be replaced by CRLF` | Просто попередження, ігноруйте. |
| `git status` показує `node_modules` | Немає `.gitignore`: додайте рядок `node_modules`. |

## 11. Шпаргалка

```
git status                     що змінилося
git pull                       забрати зміни з GitHub
git add .                      додати все до коміту
git commit -m "текст"          зберегти зміни локально
git push                       відправити на GitHub
git pull --rebase              підтягнути чужі коміти перед push
git tag v0.1.1                 створити тег
git push origin v0.1.1         відправити тег (запускає збірку програми)
git log --oneline              історія комітів
git restore файл               скасувати ваші зміни у файлі (ще не закомічені)
git revert НОМЕР               скасувати коміт новим комітом
```
