//! Транзакційне встановлення патча. Не залежить від Tauri, тому тестується окремо.
//!
//! Схема: перед змінами пишемо журнал (що існувало), перед перезаписом кожного файлу
//! кладемо його копію в `undo`, новий файл пишемо в тимчасовий і підміняємо перейменуванням.
//! Будь-яка помилка чи скасування повертає все як було. Якщо процес убили посеред
//! встановлення, `recover` при наступному запуску робить те саме по журналу.

use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct FileRec {
    /// чи існував файл у грі до нашого патча (тоді оригінал лежить у backups)
    pub had_original: bool,
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Receipt {
    pub version: String,
    /// збірка гри (Steam buildid) на момент встановлення
    #[serde(default)]
    pub build: Option<String>,
    pub files: HashMap<String, FileRec>,
}

#[derive(Serialize, Default, Debug)]
pub struct Report {
    pub missing: Vec<String>,
    pub changed: Vec<String>,
}

#[derive(Debug)]
pub enum InstallError {
    Locked(String),
    NoSpace { need: u64, free: u64 },
    Cancelled,
    Other(String),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked(r) => write!(f, "LOCKED:{r}"),
            Self::NoSpace { need, free } => write!(f, "NOSPACE:{need}:{free}"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::Other(m) => write!(f, "{m}"),
        }
    }
}

impl From<io::Error> for InstallError {
    fn from(e: io::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<zip::result::ZipError> for InstallError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Other(e.to_string())
    }
}

#[derive(Serialize, Deserialize)]
struct JEntry {
    rel: String,
    existed: bool,
}

pub fn sha256_file(p: &Path) -> io::Result<String> {
    let mut f = File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn is_locked(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
        || (cfg!(windows) && matches!(e.raw_os_error(), Some(32) | Some(33)))
}

fn tmp_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".lichtar-new");
    PathBuf::from(s)
}

fn ensure_parent(p: &Path) -> io::Result<()> {
    match p.parent() {
        Some(d) => fs::create_dir_all(d),
        None => Ok(()),
    }
}

fn rollback_txn(game_dir: &Path, undo_dir: &Path) -> io::Result<()> {
    let Ok(txt) = fs::read_to_string(undo_dir.join("journal.json")) else {
        let _ = fs::remove_dir_all(undo_dir);
        return Ok(());
    };
    let entries: Vec<JEntry> = serde_json::from_str(&txt).unwrap_or_default();
    let mut first_err: Option<io::Error> = None;
    for en in entries {
        let dest = game_dir.join(&en.rel);
        let _ = fs::remove_file(tmp_path(&dest));
        let r = if en.existed {
            let u = undo_dir.join("files").join(&en.rel);
            if u.is_file() {
                fs::copy(&u, &dest).map(|_| ())
            } else {
                Ok(())
            }
        } else {
            match fs::remove_file(&dest) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        };
        if let Err(e) = r {
            first_err.get_or_insert(e);
        }
    }
    match first_err {
        None => {
            let _ = fs::remove_dir_all(undo_dir);
            Ok(())
        }
        Some(e) => Err(e),
    }
}

/// Викликати перед встановленням: якщо минулий запуск обірвався, повертає гру до початкового стану.
pub fn recover(game_dir: &Path, undo_dir: &Path) -> io::Result<bool> {
    if !undo_dir.join("journal.json").is_file() {
        return Ok(false);
    }
    rollback_txn(game_dir, undo_dir).map(|_| true)
}

#[allow(clippy::too_many_arguments)]
pub fn install(
    zip_path: &Path,
    game_dir: &Path,
    backup_dir: &Path,
    undo_dir: &Path,
    prev: &Receipt,
    version: &str,
    build: Option<String>,
    progress: &dyn Fn(f32),
    cancel: &AtomicBool,
) -> Result<Receipt, InstallError> {
    let mut ar = zip::ZipArchive::new(File::open(zip_path)?)?;
    let mut entries: Vec<(usize, String, PathBuf, u64)> = vec![];
    for i in 0..ar.len() {
        let f = ar.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        // enclosed_name відкидає ../ та абсолютні шляхи
        let Some(rel) = f.enclosed_name() else { continue };
        entries.push((i, rel.to_string_lossy().replace('\\', "/"), rel, f.size()));
    }
    if entries.is_empty() {
        return Err(InstallError::Other("Архів порожній".into()));
    }

    // 1. перевірки до будь-яких змін
    let need = entries.iter().map(|e| e.3).sum::<u64>() * 2 + 16 * 1024 * 1024;
    let free = fs2::available_space(game_dir).unwrap_or(u64::MAX);
    if free < need {
        return Err(InstallError::NoSpace { need, free });
    }
    for (_, key, rel, _) in &entries {
        let d = game_dir.join(rel);
        if d.is_dir() {
            return Err(InstallError::Other(format!("{key}: на цьому місці тека")));
        }
        if d.exists() {
            if let Err(e) = OpenOptions::new().write(true).open(&d) {
                return Err(if is_locked(&e) { InstallError::Locked(key.clone()) } else { e.into() });
            }
        }
    }

    // 2. журнал
    let _ = fs::remove_dir_all(undo_dir);
    fs::create_dir_all(undo_dir.join("files"))?;
    let journal: Vec<JEntry> = entries
        .iter()
        .map(|(_, k, r, _)| JEntry { rel: k.clone(), existed: game_dir.join(r).exists() })
        .collect();
    let jtxt = serde_json::to_string(&journal).map_err(|e| InstallError::Other(e.to_string()))?;
    fs::write(undo_dir.join("journal.json"), jtxt)?;

    // 3. застосування
    let result: Result<HashMap<String, FileRec>, InstallError> = (|| {
        let mut files = prev.files.clone();
        let total = entries.len().max(1);
        for (n, (idx, key, rel, _)) in entries.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err(InstallError::Cancelled);
            }
            let dest = game_dir.join(rel);
            let existed = dest.exists();
            if existed {
                let u = undo_dir.join("files").join(rel);
                ensure_parent(&u)?;
                fs::copy(&dest, &u)?;
                if !prev.files.contains_key(key) {
                    let b = backup_dir.join(rel);
                    if !b.exists() {
                        ensure_parent(&b)?;
                        fs::copy(&dest, &b)?;
                    }
                }
            }
            ensure_parent(&dest)?;
            let tmp = tmp_path(&dest);
            let mut f = ar.by_index(*idx)?;
            let mut out = File::create(&tmp)?;
            let mut h = Sha256::new();
            let mut buf = [0u8; 65536];
            loop {
                let k = f.read(&mut buf)?;
                if k == 0 {
                    break;
                }
                h.update(&buf[..k]);
                out.write_all(&buf[..k])?;
            }
            out.flush()?;
            drop(out);
            fs::rename(&tmp, &dest)?;
            let had = prev.files.get(key).map(|r| r.had_original).unwrap_or(existed);
            files.insert(key.clone(), FileRec { had_original: had, sha256: hex::encode(h.finalize()) });
            progress((n + 1) as f32 / total as f32 * 100.0);
        }
        Ok(files)
    })();

    match result {
        Ok(files) => {
            let _ = fs::remove_dir_all(undo_dir);
            Ok(Receipt { version: version.to_string(), build, files })
        }
        Err(e) => {
            let _ = rollback_txn(game_dir, undo_dir);
            Err(e)
        }
    }
}

/// Відкат до оригіналу за квитанцією.
pub fn restore_original(game_dir: &Path, backup_dir: &Path, r: &Receipt) -> Result<(), String> {
    let mut errs = vec![];
    for (rel, rec) in &r.files {
        let dest = game_dir.join(rel);
        let res = if rec.had_original {
            let b = backup_dir.join(rel);
            if b.is_file() {
                fs::copy(&b, &dest).map(|_| ())
            } else {
                Err(io::Error::new(io::ErrorKind::NotFound, "немає резервної копії"))
            }
        } else {
            match fs::remove_file(&dest) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        };
        if let Err(e) = res {
            errs.push(format!("{rel}: {e}"));
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs.join("; ")) }
}

/// Чи досі на місці файли українізатора (не видалені Steam-оновленням, не змінені).
pub fn verify(game_dir: &Path, r: &Receipt) -> Report {
    let mut rep = Report::default();
    for (rel, rec) in &r.files {
        let p = game_dir.join(rel);
        if !p.is_file() {
            rep.missing.push(rel.clone());
        } else if sha256_file(&p).map(|h| !h.eq_ignore_ascii_case(&rec.sha256)).unwrap_or(true) {
            rep.changed.push(rel.clone());
        }
    }
    rep.missing.sort();
    rep.changed.sort();
    rep
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, path::Path, sync::atomic::{AtomicBool, Ordering}};

    fn make_zip(p: &Path, files: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(fs::File::create(p).unwrap());
        for (n, c) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    fn read(p: &Path) -> String { fs::read_to_string(p).unwrap() }

    #[test]
    fn install_update_verify_rollback() {
        let t = tempfile::tempdir().unwrap();
        let (game, backup, undo) = (t.path().join("game"), t.path().join("bk"), t.path().join("undo"));
        fs::create_dir_all(game.join("Data")).unwrap();
        fs::write(game.join("Data/lang.txt"), "ORIG").unwrap();
        let z1 = t.path().join("p1.zip");
        make_zip(&z1, &[("Data/lang.txt", "UA1"), ("Data/new.txt", "NEW"), ("../evil.txt", "x")]);
        let no = AtomicBool::new(false);
        let r1 = install(&z1, &game, &backup, &undo, &Receipt::default(), "1.0", Some("111".into()), &|_| {}, &no).unwrap();
        assert_eq!(read(&game.join("Data/lang.txt")), "UA1");
        assert_eq!(read(&backup.join("Data/lang.txt")), "ORIG");
        assert!(!t.path().join("evil.txt").exists(), "zip-slip");
        assert!(r1.files["Data/lang.txt"].had_original && !r1.files["Data/new.txt"].had_original);
        assert!(!undo.exists());
        assert!(verify(&game, &r1).changed.is_empty());

        // оновлення 1.0 -> 2.0 зберігає оригінал у backups
        let z2 = t.path().join("p2.zip");
        make_zip(&z2, &[("Data/lang.txt", "UA2")]);
        let r2 = install(&z2, &game, &backup, &undo, &r1, "2.0", None, &|_| {}, &no).unwrap();
        assert_eq!(read(&backup.join("Data/lang.txt")), "ORIG");
        assert!(r2.files["Data/lang.txt"].had_original && r2.files.contains_key("Data/new.txt"));

        // змінений файл помічається
        fs::write(game.join("Data/lang.txt"), "TAMPER").unwrap();
        assert_eq!(verify(&game, &r2).changed, vec!["Data/lang.txt"]);
        fs::remove_file(game.join("Data/new.txt")).unwrap();
        assert_eq!(verify(&game, &r2).missing, vec!["Data/new.txt"]);

        // відкат до оригіналу
        restore_original(&game, &backup, &r2).unwrap();
        assert_eq!(read(&game.join("Data/lang.txt")), "ORIG");
        assert!(!game.join("Data/new.txt").exists());
    }

    #[test]
    fn cancel_midway_restores_everything() {
        let t = tempfile::tempdir().unwrap();
        let (game, backup, undo) = (t.path().join("game"), t.path().join("bk"), t.path().join("undo"));
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("a.txt"), "A0").unwrap();
        fs::write(game.join("b.txt"), "B0").unwrap();
        let z = t.path().join("p.zip");
        make_zip(&z, &[("a.txt", "A1"), ("b.txt", "B1"), ("c.txt", "C1")]);
        let cancel = AtomicBool::new(false);
        let r = install(&z, &game, &backup, &undo, &Receipt::default(), "1", None, &|p| { if p > 50.0 { cancel.store(true, Ordering::Relaxed) } }, &cancel);
        assert_eq!(r.unwrap_err().to_string(), "CANCELLED");
        assert_eq!(read(&game.join("a.txt")), "A0");
        assert_eq!(read(&game.join("b.txt")), "B0");
        assert!(!game.join("c.txt").exists());
        assert!(!undo.exists());
    }

    #[test]
    fn crash_recovery_from_journal() {
        let t = tempfile::tempdir().unwrap();
        let (game, backup, undo) = (t.path().join("game"), t.path().join("bk"), t.path().join("undo"));
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("a.txt"), "A0").unwrap();
        let z = t.path().join("p.zip");
        make_zip(&z, &[("a.txt", "A1"), ("n.txt", "N1")]);
        let cancel = AtomicBool::new(false);
        // імітуємо падіння: ставимо патч, потім повертаємо журнал і undo як їх лишив би обірваний процес
        install(&z, &game, &backup, &undo, &Receipt::default(), "1", None, &|_| {}, &cancel).unwrap();
        fs::create_dir_all(undo.join("files")).unwrap();
        fs::write(undo.join("files/a.txt"), "A0").unwrap();
        fs::write(undo.join("journal.json"), r#"[{"rel":"a.txt","existed":true},{"rel":"n.txt","existed":false}]"#).unwrap();
        assert!(recover(&game, &undo).unwrap());
        assert_eq!(read(&game.join("a.txt")), "A0");
        assert!(!game.join("n.txt").exists());
        assert!(!recover(&game, &undo).unwrap());
    }

    #[test]
    fn empty_and_not_enough_space() {
        let t = tempfile::tempdir().unwrap();
        let game = t.path().join("g"); fs::create_dir_all(&game).unwrap();
        let z = t.path().join("e.zip"); make_zip(&z, &[]);
        let c = AtomicBool::new(false);
        assert!(install(&z, &game, &t.path().join("b"), &t.path().join("u"), &Receipt::default(), "1", None, &|_| {}, &c).is_err());
    }
}
