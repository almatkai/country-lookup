//! Bounded variant of pinned SDK storage semantics using only SDK-granted paths.
//! The pinned read_store uses an unbounded fs::read; retain its atomic transaction
//! semantics here while enforcing limits before allocation and parsing.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
};
use tezbar_extension_sdk::ExtensionContext;
static LANE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
fn read(path: &Path, max: usize) -> Result<Value, String> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() || m.len() > max as u64 => {
            return Err("Private storage is invalid or too large".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(_) => return Err("Private storage is unreadable".into()),
        _ => {}
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "Private storage is unreadable")?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Private storage is unreadable")?;
    if bytes.len() > max {
        return Err("Private storage is too large".into());
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|_| "Private storage is corrupt")?;
    if !v.is_object() {
        return Err("Private storage must be an object".into());
    }
    Ok(v)
}
pub async fn transaction<F>(
    ctx: &ExtensionContext,
    name: &str,
    max: usize,
    change: F,
) -> Result<Value, String>
where
    F: FnOnce(&mut Value) -> Result<bool, String>,
{
    let _lane = LANE.lock().await;
    if !matches!(name, "country-lookup" | "country-lookup-cache") {
        return Err("Invalid private storage name".into());
    }
    let dir = ctx.paths.data_dir.join(name);
    if let Ok(m) = std::fs::symlink_metadata(&dir) {
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err("Private storage directory is invalid".into());
        }
    }
    let path = dir.join("storage.json");
    let mut v = read(&path, max)?;
    if change(&mut v)? {
        let bytes = serde_json::to_vec(&v).map_err(|_| "Invalid private storage")?;
        if !v.is_object() || bytes.len() > max {
            return Err("Private storage is too large or invalid".into());
        }
        std::fs::create_dir_all(&dir).map_err(|_| "Could not create private storage")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| "Could not protect private storage")?;
        }
        let mut tmp = tempfile::NamedTempFile::new_in(&dir)
            .map_err(|_| "Could not create private storage transaction")?;
        tmp.write_all(&bytes)
            .and_then(|_| tmp.as_file().sync_all())
            .map_err(|_| "Could not write private storage transaction")?;
        tmp.persist(&path)
            .map_err(|_| "Could not commit private storage transaction")?;
        #[cfg(unix)]
        std::fs::File::open(&dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| "Could not sync private storage")?;
    }
    Ok(v)
}
