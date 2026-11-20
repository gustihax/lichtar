<div align="center">

<img src=".github/readme/banner.webp" alt="Ліхтар — українізатори ігор в один клік" width="100%">

<br>

<a href="https://github.com/gustihax/lichtar/releases/latest"><img src="https://img.shields.io/github/v/release/gustihax/lichtar?style=flat-square&color=ffb000&labelColor=1b1916&label=%D0%B2%D0%B5%D1%80%D1%81%D1%96%D1%8F" alt="Версія"></a>
<a href="https://github.com/gustihax/lichtar/releases"><img src="https://img.shields.io/github/downloads/gustihax/lichtar/total?style=flat-square&color=ffb000&labelColor=1b1916&label=%D0%B7%D0%B0%D0%B2%D0%B0%D0%BD%D1%82%D0%B0%D0%B6%D0%B5%D0%BD%D1%8C" alt="Завантажень"></a>
<img src="https://img.shields.io/badge/Windows-10%20%2F%2011-ffb000?style=flat-square&labelColor=1b1916" alt="Windows 10 / 11">
<a href="LICENSE"><img src="https://img.shields.io/github/license/gustihax/lichtar?style=flat-square&color=ffb000&labelColor=1b1916&label=%D0%BB%D1%96%D1%86%D0%B5%D0%BD%D0%B7%D1%96%D1%8F" alt="Ліцензія"></a>

<br><br>

<a href="https://github.com/gustihax/lichtar/releases/latest"><img src="https://img.shields.io/badge/%D0%97%D0%B0%D0%B2%D0%B0%D0%BD%D1%82%D0%B0%D0%B6%D0%B8%D1%82%D0%B8%20%D0%B4%D0%BB%D1%8F%20Windows-ffb000?style=for-the-badge&logoColor=1a1200" alt="Завантажити для Windows" height="42"></a>

<br>

**[Сайт](https://gustihax.github.io/lichtar/)** &nbsp;·&nbsp; **[Каталог перекладів](https://github.com/gustihax/lichtar-catalog)** &nbsp;·&nbsp; **[Хочу переклад](https://github.com/gustihax/lichtar-catalog/issues?q=is%3Aissue+is%3Aopen+label%3Awish)** &nbsp;·&nbsp; **[Повідомити про проблему](https://github.com/gustihax/lichtar/issues)**

</div>

<br>

Ліхтар — безкоштовна програма для Windows, яка ставить українські переклади ігор. Не треба шукати архіви по форумах, читати інструкції й копіювати файли в теки гри: Ліхтар сам знаходить встановлені ігри, ставить переклад одним натиском і повідомляє, коли вийшла нова версія.

Переклади готує спільнота **Тлумачі Світла**.

<br>

<img src=".github/readme/home.webp" alt="Головний екран Ліхтаря" width="100%">

<table>
<tr>
<td width="50%"><img src=".github/readme/game.webp" alt="Сторінка гри"></td>
<td width="50%"><img src=".github/readme/news.webp" alt="Стрічка новин"></td>
</tr>
<tr>
<td align="center"><sub>Сторінка гри: встановлення, історія змін, де лежить гра</sub></td>
<td align="center"><sub>Новини: релізи перекладів і події навколо українізації</sub></td>
</tr>
</table>

## Що вміє

<table>
<tr>
<td width="50%" valign="top">

**Знаходить ігри сам**<br>
Steam, GOG та Epic Games. Якщо гра стоїть деінде, теку можна вказати вручну.

</td>
<td width="50%" valign="top">

**Безпечне встановлення**<br>
Перед змінами Ліхтар зберігає оригінальні файли. Гру завжди можна повернути до початкового стану.

</td>
</tr>
<tr>
<td valign="top">

**Оновлення в один клік**<br>
Вийшла нова версія перекладу — на плитці гри з'явиться позначка, а «Оновити все» поставить усе разом.

</td>
<td valign="top">

**Перевірка цілісності**<br>
Кожен архів перевіряється за контрольною сумою SHA-256 до того, як потрапить у гру.

</td>
</tr>
<tr>
<td valign="top">

**«Повідомити, коли вийде»**<br>
Для перекладів, які ще в роботі, видно прогрес. Підпишіться, і Ліхтар скаже, коли переклад з'явиться.

</td>
<td valign="top">

**Новини та «Хочу переклад»**<br>
Стрічка про українізатори оновлюється сама. А гру, яку хочете бачити українською, можна запропонувати й підтримати голосом.

</td>
</tr>
</table>

## До і після

На сторінці гри можна порівняти оригінал і переклад, перетягуючи повзунок.

<img src=".github/readme/compare.webp" alt="Gunbrella: оригінал і українська версія" width="100%">

## Встановлення

1. Завантажте інсталятор з **[останнього релізу](https://github.com/gustihax/lichtar/releases/latest)** (файл `.exe`).
2. Запустіть його. Встановлення не потребує прав адміністратора.
3. Відкрийте Ліхтар, оберіть гру й натисніть **«Встановити»**.

> [!NOTE]
> Якщо Windows показала вікно **«Windows захистила ваш ПК»**, натисніть **«Докладніше»**, потім **«Все одно виконати»**. Це SmartScreen: він попереджає про будь-яку нову програму без платного сертифіката видавця. Увесь код Ліхтаря відкритий, а кожен реліз збирається публічно в [GitHub Actions](https://github.com/gustihax/lichtar/actions).

Далі Ліхтар оновлюється сам: коли вийде нова версія, у шапці з'явиться кнопка оновлення, а після перезапуску ви побачите, що змінилося.

<div align="center">
<img src=".github/readme/wn.webp" alt="Вікно «Що нового» після оновлення" width="70%">
</div>

## Ігри

Список ігор і самі переклади живуть в окремому репозиторії **[lichtar-catalog](https://github.com/gustihax/lichtar-catalog)**. Ліхтар завантажує його під час запуску, тому нові переклади з'являються в програмі одразу, без її оновлення.

Не знайшли свою гру? [Запропонуйте її](https://github.com/gustihax/lichtar-catalog/issues?q=is%3Aissue+is%3Aopen+label%3Awish) або поставте 👍 тій, що вже є в списку: так ми бачимо, що перекладати далі.

**Перекладачам:** як додати свою гру чи випустити оновлення патча, описано в [інструкції для мейнтейнерів](https://github.com/gustihax/lichtar-catalog/blob/main/docs/maintainers.md).

## Ліцензія

[MIT](LICENSE). Програма безкоштовна й такою залишиться. Переклади належать їхнім авторам.

<br>

<div align="center">
<sub>Зроблено з любов'ю до української локалізації · Тлумачі Світла</sub>
</div>
