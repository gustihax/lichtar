mod download;
mod installer;

use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use installer::Receipt;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let p = app.path().app_data_dir().map_err(e)?;
    fs::create_dir_all(&p).map_err(e)?;
    Ok(p)
}

// ---------- журнал подій ----------

fn log(app: &AppHandle, msg: &str) {
    let Ok(d) = data_dir(app) else { return };
    let p = d.join("lichtar.log");
    if fs::metadata(&p).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
        let _ = fs::rename(&p, d.join("lichtar.old.log"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    }
}

#[tauri::command]
fn log_event(app: AppHandle, msg: String) {
    log(&app, &format!("[ui] {msg}"));
}

#[tauri::command]
fn read_log(app: AppHandle) -> Result<String, String> {
    let t = fs::read_to_string(data_dir(&app)?.join("lichtar.log")).unwrap_or_default();
    let lines: Vec<&str> = t.lines().collect();
    Ok(lines[lines.len().saturating_sub(300)..].join("\n"))
}

// ---------- квитанції ----------

fn receipt_path(root: &Path, id: &str) -> PathBuf {
    root.join("receipts").join(format!("{id}.json"))
}

fn read_receipt(root: &Path, id: &str) -> Option<Receipt> {
    serde_json::from_str(&fs::read_to_string(receipt_path(root, id)).ok()?).ok()
}

#[derive(Serialize)]
struct Installed {
    version: String,
    build: Option<String>,
}

#[tauri::command]
fn installed_versions(app: AppHandle) -> Result<HashMap<String, Installed>, String> {
    let root = data_dir(&app)?;
    let mut out = HashMap::new();
    if let Ok(rd) = fs::read_dir(root.join("receipts")) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|x| x == "json") {
                if let Some(id) = p.file_stem().and_then(|s| s.to_str()) {
                    if let Some(r) = read_receipt(&root, id) {
                        out.insert(id.to_string(), Installed { version: r.version, build: r.build });
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Чи досі на місці файли українізатора (Steam міг оновити гру і стерти їх).
#[tauri::command]
async fn verify_install(app: AppHandle, id: String, game_dir: String) -> Result<installer::Report, String> {
    let root = data_dir(&app)?;
    let Some(r) = read_receipt(&root, &id) else { return Ok(Default::default()) };
    tauri::async_runtime::spawn_blocking(move || installer::verify(Path::new(&game_dir), &r))
        .await
        .map_err(e)
}

// ---------- Steam ----------

/// Steam і реєстр віддають шляхи малими літерами й зі скісними рисками (c:/program files/...).
/// Беремо справжній запис з диска: canonicalize повертає регістр так, як тека названа у Windows.
fn pretty_path(p: &Path) -> String {
    let raw = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().into_owned();
    strip_verbatim(&raw)
}

fn strip_verbatim(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = s.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

#[cfg(windows)]
fn steam_root() -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok()?;
    let p: String = key.get_value("SteamPath").ok()?;
    Some(PathBuf::from(p))
}

#[cfg(not(windows))]
fn steam_root() -> Option<PathBuf> {
    None
}

/// Друге значення в рядку VDF: "key"   "value"
fn vdf_value(line: &str) -> Option<String> {
    line.split('"').nth(3).map(|s| s.replace("\\\\", "\\"))
}

fn steam_libraries(root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![root.to_path_buf()];
    if let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
        for line in text.lines().filter(|l| l.contains("\"path\"")) {
            if let Some(p) = vdf_value(line) {
                libs.push(PathBuf::from(p));
            }
        }
    }
    libs
}

fn steam_manifest(appid: u32) -> Option<(PathBuf, String)> {
    let root = steam_root()?;
    for lib in steam_libraries(&root) {
        if let Ok(t) = fs::read_to_string(lib.join(format!("steamapps/appmanifest_{appid}.acf"))) {
            return Some((lib, t));
        }
    }
    None
}

fn acf_value(text: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    text.lines().find(|l| l.contains(&pat)).and_then(vdf_value)
}

fn steam_build(appid: u32) -> Option<String> {
    steam_manifest(appid).and_then(|(_, t)| acf_value(&t, "buildid"))
}

fn find_steam_game_dir(appid: u32) -> Option<String> {
    let (lib, t) = steam_manifest(appid)?;
    let full = lib.join("steamapps/common").join(acf_value(&t, "installdir")?);
    full.is_dir().then(|| pretty_path(&full))
}

/// Поточна збірка гри в Steam (buildid). Змінюється, коли Steam оновлює гру.
#[tauri::command]
fn game_build(appid: u32) -> Option<String> {
    steam_build(appid)
}

// ---------- пошук гри, збережені шляхи, каталог ----------

/// Тека вважається грою, якщо в ній лежить один з відомих exe (так працює для будь-якої копії).
fn has_game(dir: &Path, exes: &[String]) -> bool {
    dir.is_dir() && (exes.is_empty() || exes.iter().any(|x| dir.join(x).is_file()))
}

#[tauri::command]
fn validate_game_dir(dir: String, exes: Vec<String>) -> bool {
    has_game(Path::new(&dir), &exes)
}

#[cfg(windows)]
fn gog_path(id: &str) -> Option<PathBuf> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(format!("SOFTWARE\\WOW6432Node\\GOG.com\\Games\\{id}"))
        .ok()?;
    let p: String = key.get_value("path").ok()?;
    Some(PathBuf::from(p))
}

#[cfg(not(windows))]
fn gog_path(_id: &str) -> Option<PathBuf> {
    None
}

fn detect_blocking(appid: Option<u32>, gog_id: Option<String>, exes: Vec<String>, folders: Vec<String>) -> Option<String> {
    if let Some(p) = appid.and_then(find_steam_game_dir) {
        if has_game(Path::new(&p), &exes) {
            return Some(p);
        }
    }
    if let Some(p) = gog_id.as_deref().and_then(gog_path) {
        if has_game(&p, &exes) {
            return Some(pretty_path(&p));
        }
    }
    if exes.is_empty() {
        return None;
    }
    let bases = [
        "",
        "Games",
        "Game",
        "Program Files",
        "Program Files (x86)",
        "Epic Games",
        "GOG Games",
        "SteamLibrary\\steamapps\\common",
        "Steam\\steamapps\\common",
    ];
    for letter in 'C'..='Z' {
        let root = PathBuf::from(format!("{letter}:\\"));
        if !root.exists() {
            continue;
        }
        for base in bases {
            for f in &folders {
                let p = root.join(base).join(f);
                if has_game(&p, &exes) {
                    return Some(pretty_path(&p));
                }
            }
        }
    }
    None
}

/// Пошук по дисках може бути довгим, тому не на головному потоці.
#[tauri::command]
async fn detect_game(
    appid: Option<u32>,
    gog_id: Option<String>,
    exes: Vec<String>,
    folders: Vec<String>,
) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || detect_blocking(appid, gog_id, exes, folders))
        .await
        .ok()
        .flatten()
}

fn paths_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("paths.json"))
}

#[tauri::command]
fn get_paths(app: AppHandle) -> Result<HashMap<String, String>, String> {
    let mut m: HashMap<String, String> = fs::read_to_string(paths_file(&app)?)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // Раніше збережені шляхи могли бути малими літерами: виправляємо при читанні.
    for v in m.values_mut() {
        if Path::new(v.as_str()).exists() {
            *v = pretty_path(Path::new(v.as_str()));
        }
    }
    Ok(m)
}

#[tauri::command]
fn set_path(app: AppHandle, id: String, path: String) -> Result<(), String> {
    let mut m = get_paths(app.clone())?;
    m.insert(id, path);
    fs::write(paths_file(&app)?, serde_json::to_string_pretty(&m).map_err(e)?).map_err(e)
}

#[tauri::command]
async fn fetch_text(url: String) -> Result<String, String> {
    reqwest::Client::builder()
        .user_agent("Lichtar/0.1")
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(e)?
        .get(url)
        .send()
        .await
        .map_err(e)?
        .error_for_status()
        .map_err(e)?
        .text()
        .await
        .map_err(e)
}

/// Анонімна статистика (за згодою користувача, адресу задає JS).
#[tauri::command]
async fn post_json(url: String, body: String) -> Result<(), String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(e)?
        .post(url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(e)?
        .error_for_status()
        .map_err(e)?;
    Ok(())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    let path = app.dialog().file().blocking_pick_folder()?;
    path.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}

/// Steam-копія запускається через Steam, усі інші (GOG, Epic, репаки) напряму через exe.
#[tauri::command]
fn play_game(app: AppHandle, appid: Option<u32>, game_dir: String, exe: Option<String>) -> Result<(), String> {
    if let Some(id) = appid {
        if game_dir.to_lowercase().contains("steamapps") {
            return app
                .opener()
                .open_url(format!("steam://rungameid/{id}"), None::<&str>)
                .map_err(e);
        }
    }
    let exe = exe.ok_or("У каталозі не вказано exe для запуску")?;
    let path = Path::new(&game_dir).join(&exe);
    if !path.is_file() {
        return Err(format!("Не знайдено {exe}"));
    }
    std::process::Command::new(path).current_dir(&game_dir).spawn().map(|_| ()).map_err(e)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:")) {
        return Err("Некоректне посилання".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(e)
}

#[tauri::command]
fn open_folder(app: AppHandle, path: String) -> Result<(), String> {
    app.opener().open_path(path, None::<&str>).map_err(e)
}

// ---------- сховище ----------

fn dir_size(p: &Path) -> u64 {
    fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .map(|en| {
                    let q = en.path();
                    if q.is_dir() { dir_size(&q) } else { en.metadata().map(|m| m.len()).unwrap_or(0) }
                })
                .sum()
        })
        .unwrap_or(0)
}

#[derive(Serialize)]
struct Storage {
    dir: String,
    log: String,
    backups: u64,
    cache: u64,
}

#[tauri::command]
fn storage_info(app: AppHandle) -> Result<Storage, String> {
    let r = data_dir(&app)?;
    Ok(Storage {
        dir: r.to_string_lossy().into_owned(),
        log: r.join("lichtar.log").to_string_lossy().into_owned(),
        backups: dir_size(&r.join("backups")),
        cache: dir_size(&r.join("tmp")),
    })
}

#[tauri::command]
fn clear_cache(app: AppHandle) -> Result<u64, String> {
    let t = data_dir(&app)?.join("tmp");
    let n = dir_size(&t);
    let _ = fs::remove_dir_all(&t);
    Ok(n)
}

// ---------- встановлення ----------

#[derive(Clone, Serialize)]
struct Progress {
    id: String,
    phase: String,
    pct: f32,
}

fn emit(app: &AppHandle, id: &str, phase: &str, pct: f32) {
    let _ = app.emit("progress", Progress { id: id.into(), phase: phase.into(), pct });
}

#[derive(Default)]
struct Jobs(Mutex<HashMap<String, Arc<AtomicBool>>>);

#[derive(Default)]
struct StartupLink(Mutex<Option<String>>);

#[tauri::command]
fn cancel_install(jobs: State<'_, Jobs>, id: String) {
    if let Some(f) = jobs.0.lock().unwrap().get(&id) {
        f.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn startup_link(s: State<'_, StartupLink>) -> Option<String> {
    s.0.lock().unwrap().take()
}

#[allow(clippy::too_many_arguments)]
async fn do_install(
    app: &AppHandle,
    id: &str,
    version: &str,
    url: &str,
    sha256: &str,
    game_dir: &str,
    appid: Option<u32>,
    flag: Arc<AtomicBool>,
) -> Result<(), String> {
    let root = data_dir(app)?;
    let tmp = root.join("tmp");
    fs::create_dir_all(&tmp).map_err(e)?;
    let game = PathBuf::from(game_dir);
    if !game.is_dir() {
        return Err("Теку гри не знайдено".into());
    }
    let undo = root.join("undo").join(id);
    let backup = root.join("backups").join(id);
    if installer::recover(&game, &undo).map_err(e)? {
        log(app, &format!("{id}: відновлено стан після обірваного встановлення"));
    }

    // 1. завантаження з докачуванням (див. download.rs)
    let part = tmp.join(format!("{id}-{version}.part"));
    let (app_p, id_p) = (app.clone(), id.to_string());
    download::download(url, &part, &flag, &move |pct| emit(&app_p, &id_p, "download", pct))
        .await
        .map_err(|er| er.to_string())?;

    // 2. перевірка SHA256
    emit(app, id, "verify", 100.0);
    let p2 = part.clone();
    let got_hash = tauri::async_runtime::spawn_blocking(move || installer::sha256_file(&p2))
        .await
        .map_err(e)?
        .map_err(e)?;
    if !got_hash.eq_ignore_ascii_case(sha256) {
        let _ = fs::remove_file(&part);
        return Err("SHA256 не збігається: файл пошкоджено або підмінено".into());
    }

    // 3. транзакційне встановлення
    let prev = read_receipt(&root, id).unwrap_or_default();
    let build = if game_dir.to_lowercase().contains("steamapps") { appid.and_then(steam_build) } else { None };
    let (app2, id2, flag2, part2, ver) = (app.clone(), id.to_string(), flag.clone(), part.clone(), version.to_string());
    let receipt = tauri::async_runtime::spawn_blocking(move || {
        installer::install(&part2, &game, &backup, &undo, &prev, &ver, build, &|p| emit(&app2, &id2, "extract", p), &flag2)
    })
    .await
    .map_err(e)?
    .map_err(|er| er.to_string())?;

    let rp = receipt_path(&root, id);
    fs::create_dir_all(rp.parent().unwrap()).map_err(e)?;
    fs::write(&rp, serde_json::to_string_pretty(&receipt).map_err(e)?).map_err(e)?;
    let _ = fs::remove_file(&part);
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn install_patch(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    id: String,
    version: String,
    url: String,
    sha256: String,
    game_dir: String,
    appid: Option<u32>,
) -> Result<(), String> {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut m = jobs.0.lock().unwrap();
        if m.contains_key(&id) {
            return Err("Встановлення вже триває".into());
        }
        m.insert(id.clone(), flag.clone());
    }
    log(&app, &format!("install {id} {version} -> {game_dir}"));
    let res = do_install(&app, &id, &version, &url, &sha256, &game_dir, appid, flag).await;
    jobs.0.lock().unwrap().remove(&id);
    match &res {
        Ok(()) => log(&app, &format!("install {id} {version}: ok")),
        Err(er) => log(&app, &format!("install {id} {version}: {er}")),
    }
    res
}

/// Якщо програму закрили посеред встановлення, при запуску повертаємо гру до початкового стану.
#[tauri::command]
async fn recover_pending(app: AppHandle) -> Result<Vec<String>, String> {
    let root = data_dir(&app)?;
    let paths = get_paths(app.clone())?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut done = vec![];
        for (id, dir) in paths {
            let undo = root.join("undo").join(&id);
            if installer::recover(Path::new(&dir), &undo).unwrap_or(false) {
                done.push(id);
            }
        }
        done
    })
    .await
    .map_err(e)
}

#[tauri::command]
async fn rollback(app: AppHandle, id: String, game_dir: String) -> Result<(), String> {
    let root = data_dir(&app)?;
    let Some(r) = read_receipt(&root, &id) else {
        return Err("Немає даних про встановлення".into());
    };
    let backup = root.join("backups").join(&id);
    let (gd, b2) = (PathBuf::from(game_dir), backup.clone());
    tauri::async_runtime::spawn_blocking(move || installer::restore_original(&gd, &b2, &r))
        .await
        .map_err(e)??;
    let _ = fs::remove_file(receipt_path(&root, &id));
    let _ = fs::remove_dir_all(backup);
    log(&app, &format!("rollback {id}: ok"));
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    // Самооновлення вмикається лише тоді, коли в tauri.conf.json є розділ plugins.updater.
    // Без нього програма просто працює без оновлень, нічого коментувати в коді не треба.
    let with_updater = context.config().plugins.0.contains_key("updater");

    let mut builder = tauri::Builder::default()
        // single-instance має бути першим плагіном
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    if with_updater {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .manage(Jobs::default())
        .manage(StartupLink::default())
        .setup(|app| {
            #[cfg(debug_assertions)]
            let _ = app.deep_link().register_all();
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                if let Some(u) = urls.first() {
                    *app.state::<StartupLink>().0.lock().unwrap() = Some(u.to_string());
                }
            }
            let h = app.handle().clone();
            app.deep_link().on_open_url(move |ev| {
                for u in ev.urls() {
                    let _ = h.emit("deep-link", u.to_string());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            find_steam_game_cmd,
            game_build,
            pick_folder,
            play_game,
            open_folder,
            installed_versions,
            verify_install,
            install_patch,
            cancel_install,
            rollback,
            fetch_text,
            post_json,
            detect_game,
            validate_game_dir,
            get_paths,
            set_path,
            storage_info,
            clear_cache,
            log_event,
            read_log,
            startup_link,
            open_url,
            recover_pending
        ])
        .run(context)
        .expect("не вдалося запустити Ліхтар");
}

#[tauri::command]
fn find_steam_game_cmd(appid: u32) -> Option<String> {
    find_steam_game_dir(appid)
}

#[cfg(test)]
mod path_tests {
    use super::strip_verbatim;

    #[test]
    fn verbatim_prefix_is_removed() {
        assert_eq!(strip_verbatim("\\\\?\\C:\\Games\\Blood West"), "C:\\Games\\Blood West");
        assert_eq!(strip_verbatim("\\\\?\\UNC\\srv\\share\\g"), "\\\\srv\\share\\g");
        assert_eq!(strip_verbatim("D:\\Steam"), "D:\\Steam");
    }
}
