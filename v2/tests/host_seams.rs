#![allow(dead_code, unused_variables, clippy::redundant_closure, clippy::nonminimal_bool)]
// Generated from host ea100cee; only dependency seams below are synthetic.
mod v2_runtime {pub fn canonical_id(slug:&str)->String{format!("com.tezbar.native.{slug}")}}
mod before {
use serde_json::Value; use std::path::{Path,PathBuf,Component};
#[derive(Clone)]
pub(super) struct Installed {
    id: String,
    slug: String,
    path: PathBuf,
    manifest: Value,
    data_dir: PathBuf,
}
fn slug(id: &str) -> Result<String, String> {
    let name = id
        .trim()
        .strip_prefix("com.tezbar.native.")
        .or_else(|| id.trim().strip_prefix("raycast."))
        .unwrap_or(id.trim());
    if name.is_empty()
        || name.len() > 200
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Invalid extension id".into());
    }
    Ok(name.into())
}
pub(super) fn installed_command(ctx: &CoreContext, id: &str, command: &str) -> Option<Installed> {
    let native = if id.starts_with("raycast.") {
        let slug = slug(id).ok()?;
        let canonical = super::v2_runtime::canonical_id(&slug);
        installed(ctx, &canonical).ok()
    } else if id.starts_with("com.tezbar.native.") {
        installed(ctx, id).ok()
    } else {
        None
    };
    if let Some(extension) = native {
        let active = extension.manifest["healthError"].is_null()
            && extension.manifest["commands"]
                .as_array()?
                .iter()
                .any(|c| c["name"] == command && c["runtime"] == "rust" && compatible(c, true));
        if active {
            return Some(extension);
        }
        if id.starts_with("com.tezbar.native.") {
            return None;
        }
    }
    if id.starts_with("com.tezbar.native.") {
        return None;
    }
    let extension = installed(ctx, id).ok()?;
    (compatible(&extension.manifest, false)
        && extension.manifest["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|cmd| cmd["name"].as_str() == Some(command) && compatible(cmd, true)))
    .then_some(extension)
}
fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "macOS"
    }
}
fn compatible(manifest: &Value, command: bool) -> bool {
    if command && manifest.get("platforms").is_none() {
        return true;
    }
    let Some(platforms) = manifest["platforms"]
        .as_array()
        .or_else(|| manifest["tezbar"]["platforms"].as_array())
    else {
        return platform() == "macOS";
    };
    platforms
        .iter()
        .filter_map(Value::as_str)
        .any(|value| match value.to_lowercase().as_str() {
            "macos" | "darwin" | "mac" => platform() == "macOS",
            "windows" | "win32" | "win" => platform() == "Windows",
            "linux" => platform() == "Linux",
            _ => false,
        })
}
fn fallback<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}
fn icon_path(extension: &Installed) -> Option<String> {
    let name = fallback(&extension.manifest, "icon", "icon.png");
    if name.starts_with("https://") || name.starts_with("http://") {
        return Some(name.into());
    }
    let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let mut candidates = vec![
        extension.path.join(relative),
        extension.path.join("assets").join(relative),
    ];
    if extension.id.starts_with("com.tezbar.native.") {
        if let Some(packages) = extension.path.parent() {
            if let Some(extensions) = packages.parent() {
                let legacy = extensions.join(&extension.slug);
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
            let legacy = packages.join(format!("raycast.{}", extension.slug));
            candidates.push(legacy.join(relative));
            candidates.push(legacy.join("assets").join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into())
}
fn command_icon_path(extension: &Installed, cmd: &Value) -> Option<String> {
    let name = cmd["icon"].as_str().or_else(|| cmd["iconPath"].as_str())?;
    if name.starts_with("https://") || name.starts_with("http://") {
        return Some(name.into());
    }
    let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let mut candidates = vec![
        extension.path.join(relative),
        extension.path.join("assets").join(relative),
    ];
    if extension.id.starts_with("com.tezbar.native.") {
        if let Some(packages) = extension.path.parent() {
            if let Some(extensions) = packages.parent() {
                let legacy = extensions.join(&extension.slug);
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
            let legacy = packages.join(format!("raycast.{}", extension.slug));
            candidates.push(legacy.join(relative));
            candidates.push(legacy.join("assets").join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into())
}
#[derive(Clone)] pub struct CoreContext { pub entries:Vec<Installed> }
fn collect(ctx:&CoreContext)->Vec<Installed>{ctx.entries.clone()}
fn installed(ctx:&CoreContext,id:&str)->Result<Installed,String>{let wanted=slug(id)?;let canonical=id.starts_with("com.tezbar.native.");ctx.entries.iter().find(|e|e.slug==wanted&&e.id.starts_with("com.tezbar.native.")==canonical).cloned().ok_or("missing".into())}

pub fn probe(root:&Path,resource:&Path)->(bool,bool,bool,bool) {
 let entry=Installed{id:"com.tezbar.native.country-lookup".into(),slug:"country-lookup".into(),path:root.into(),data_dir:root.into(), manifest:serde_json::json!({"icon":"resources/extension_iconv1.png","replaces":["com.tezbar.country-lookup","raycast.country-lookup"],"commands":[{"name":"index","runtime":"rust","platforms":["macOS","Windows","Linux"]}]})};
 let icon=icon_path(&entry).is_some();let ctx=CoreContext{entries:vec![entry]};
 (icon,installed_command(&ctx,"com.tezbar.country-lookup","index").is_some(),installed_command(&ctx,"raycast.country-lookup","index").is_some(),installed_command(&ctx,"com.tezbar.other","index").is_none())
}

}
mod after {
use serde_json::Value; use std::path::{Path,PathBuf,Component};
#[derive(Clone)]
pub(super) struct Installed {
    id: String,
    slug: String,
    path: PathBuf,
    manifest: Value,
    data_dir: PathBuf,
    // Modified: exact verified active package root; preferences keep path.
    resource_root: PathBuf,
}
fn slug(id: &str) -> Result<String, String> {
    let name = id
        .trim()
        .strip_prefix("com.tezbar.native.")
        .or_else(|| id.trim().strip_prefix("raycast."))
        .unwrap_or(id.trim());
    if name.is_empty()
        || name.len() > 200
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Invalid extension id".into());
    }
    Ok(name.into())
}
pub(super) fn installed_command(ctx: &CoreContext, id: &str, command: &str) -> Option<Installed> {
    // Modified: only exact aliases from a verified installed manifest can route natively.
    let native = if id.starts_with("com.tezbar.native.") {
        installed(ctx, id).ok()
    } else {
        collect(ctx).into_iter().find(|entry| {
            entry.id.starts_with("com.tezbar.native.")
                && entry.manifest["replaces"].as_array().is_some_and(|aliases|
                    aliases.iter().any(|alias| alias.as_str() == Some(id)))
        })
    };
    if let Some(extension) = native {
        let active = extension.manifest["healthError"].is_null()
            && extension.manifest["commands"]
                .as_array()?
                .iter()
                .any(|c| c["name"] == command && c["runtime"] == "rust" && compatible(c, true));
        if active {
            return Some(extension);
        }
        if id.starts_with("com.tezbar.native.") {
            return None;
        }
    }
    if id.starts_with("com.tezbar.native.") {
        return None;
    }
    let extension = installed(ctx, id).ok()?;
    (compatible(&extension.manifest, false)
        && extension.manifest["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|cmd| cmd["name"].as_str() == Some(command) && compatible(cmd, true)))
    .then_some(extension)
}
fn platform() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "macOS"
    }
}
fn compatible(manifest: &Value, command: bool) -> bool {
    if command && manifest.get("platforms").is_none() {
        return true;
    }
    let Some(platforms) = manifest["platforms"]
        .as_array()
        .or_else(|| manifest["tezbar"]["platforms"].as_array())
    else {
        return platform() == "macOS";
    };
    platforms
        .iter()
        .filter_map(Value::as_str)
        .any(|value| match value.to_lowercase().as_str() {
            "macos" | "darwin" | "mac" => platform() == "macOS",
            "windows" | "win32" | "win" => platform() == "Windows",
            "linux" => platform() == "Linux",
            _ => false,
        })
}
fn fallback<'a>(value: &'a Value, key: &str, default: &'a str) -> &'a str {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
}
fn icon_path(extension: &Installed) -> Option<String> {
    let name = fallback(&extension.manifest, "icon", "icon.png");
    if name.starts_with("https://") || name.starts_with("http://") {
        return Some(name.into());
    }
    let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let mut candidates = vec![
        extension.resource_root.join(relative),
        extension.resource_root.join("assets").join(relative),
    ];
    if extension.id.starts_with("com.tezbar.native.") {
        if let Some(packages) = extension.path.parent() {
            if let Some(extensions) = packages.parent() {
                let legacy = extensions.join(&extension.slug);
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
            let legacy = packages.join(format!("raycast.{}", extension.slug));
            candidates.push(legacy.join(relative));
            candidates.push(legacy.join("assets").join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into())
}
fn command_icon_path(extension: &Installed, cmd: &Value) -> Option<String> {
    let name = cmd["icon"].as_str().or_else(|| cmd["iconPath"].as_str())?;
    if name.starts_with("https://") || name.starts_with("http://") {
        return Some(name.into());
    }
    let relative = Path::new(name.strip_prefix("./").unwrap_or(name));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let mut candidates = vec![
        extension.resource_root.join(relative),
        extension.resource_root.join("assets").join(relative),
    ];
    if extension.id.starts_with("com.tezbar.native.") {
        if let Some(packages) = extension.path.parent() {
            if let Some(extensions) = packages.parent() {
                let legacy = extensions.join(&extension.slug);
                candidates.push(legacy.join(relative));
                candidates.push(legacy.join("assets").join(relative));
            }
            let legacy = packages.join(format!("raycast.{}", extension.slug));
            candidates.push(legacy.join(relative));
            candidates.push(legacy.join("assets").join(relative));
        }
    }
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into())
}
#[derive(Clone)] pub struct CoreContext { pub entries:Vec<Installed> }
fn collect(ctx:&CoreContext)->Vec<Installed>{ctx.entries.clone()}
fn installed(ctx:&CoreContext,id:&str)->Result<Installed,String>{let wanted=slug(id)?;let canonical=id.starts_with("com.tezbar.native.");ctx.entries.iter().find(|e|e.slug==wanted&&e.id.starts_with("com.tezbar.native.")==canonical).cloned().ok_or("missing".into())}

pub fn probe(root:&Path,resource:&Path)->(bool,bool,bool,bool) {
 let entry=Installed{id:"com.tezbar.native.country-lookup".into(),slug:"country-lookup".into(),path:root.into(),data_dir:root.into(),resource_root:resource.into(), manifest:serde_json::json!({"icon":"resources/extension_iconv1.png","replaces":["com.tezbar.country-lookup","raycast.country-lookup"],"commands":[{"name":"index","runtime":"rust","platforms":["macOS","Windows","Linux"]}]})};
 let icon=icon_path(&entry).is_some();let ctx=CoreContext{entries:vec![entry]};
 (icon,installed_command(&ctx,"com.tezbar.country-lookup","index").is_some(),installed_command(&ctx,"raycast.country-lookup","index").is_some(),installed_command(&ctx,"com.tezbar.other","index").is_none())
}

}
mod launcher {pub fn validate(v:&serde_json::Value)->Result<(),String>{if v.get("launcherContributions").is_some() || v["commands"].as_array().unwrap().iter().any(|c|c.get("launcher").is_some()){return Err("Launcher is outside this test seam".into())}Ok(())}}
mod runtime {
use serde_json::Value;use std::{path::PathBuf,collections::HashSet};
const CAPABILITY_GROUPS: &[&str] = &[
    "clipboard",
    "shell",
    "selection",
    "window",
    "dialog",
    "screen",
    "apps",
    "settings",
    "ai",
    "storage",
    "background",
    "tray",
    "notifications",
    "safety",
    "launcher",
];

/// Closed service allowlist shared by manifest validation and runtime grants.
const METHODS: &[&str] = &[
    "host.clipboard.read",
    "host.clipboard.write",
    "host.clipboard.writeImage",
    "host.clipboard.writeFiles",
    "host.clipboard.paste",
    "host.clipboard.pasteText",
    "host.shell.open",
    "host.shell.openWithApp",
    "host.shell.reveal",
    "host.selection.read",
    "host.window.visibility",
    "host.window.suppressBlur",
    "host.dialog.confirm",
    "host.screen.capture",
    "host.apps.list",
    "host.settings.open",
    "host.ai.ask",
    "host.background.retain",
    "host.background.release",
    "host.tray.set",
    "host.tray.remove",
    "host.notifications.show",
    "host.notifications.remove",
    "host.safety.authorize",
    "host.safety.record",
    "host.launcher.publish",
];
fn valid_capability(cap: &str) -> bool {
    CAPABILITY_GROUPS.contains(&cap)
        || METHODS
            .iter()
            .any(|m| *m == cap || m.strip_prefix("host.") == Some(cap))
}
pub(super) fn component(s: &str) -> Result<(), String> {
    if s.is_empty()
        || s.len() > 200
        || !s.as_bytes()[0].is_ascii_alphanumeric()
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("Invalid package component".into());
    }
    Ok(())
}
pub(super) fn relative(s: &str) -> Result<PathBuf, String> {
    // Reject cross-platform path syntax even when verifying on Unix.
    if s.is_empty()
        || s.len() > 1024
        || s.contains(['\\', ':', '\0'])
        || s.chars().any(|c| c.is_control())
        || s.starts_with('/')
        || s.split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.ends_with([' ', '.']))
    {
        return Err(format!("Unsafe package path: {s}"));
    }
    for part in s.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(&stem.as_str())
        {
            return Err("Reserved package filename".into());
        }
    }
    Ok(PathBuf::from(s))
}
pub(super) fn validate(manifest: &Value) -> Result<(), String> {
    if manifest["schemaVersion"] != 3
        || manifest["runtime"] != "rust"
        || manifest["execution"] != "process"
        || manifest["protocolVersion"] != 1
    {
        return Err("Extension needs a schema-3 process package (protocol 1); schema-2 markers need upgrade".into());
    }
    let slug = manifest["slug"].as_str().ok_or("Missing slug")?;
    component(slug)?;
    if manifest["id"] != format!("com.tezbar.native.{slug}") {
        return Err("Package identity mismatch".into());
    }
    let version = manifest["version"].as_str().ok_or("Missing version")?;
    component(version)?;
    if let Some(min) = manifest["minHostVersion"].as_str() {
        let min = semver::Version::parse(min).map_err(|_| "Invalid minimum host version")?;
        let host = semver::Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
        if host < min {
            return Err("Package requires a newer Tezbar host".into());
        }
    }
    let caps = manifest["capabilities"]
        .as_array()
        .ok_or("Missing capabilities")?;
    if caps
        .iter()
        .any(|c| !c.as_str().is_some_and(|s| valid_capability(s)))
    {
        return Err("Unknown capability".into());
    }
    let commands = manifest["commands"].as_array().ok_or("Missing commands")?;
    let mut names = HashSet::new();
    for c in commands {
        let name = c["name"].as_str().ok_or("Missing command name")?;
        component(name)?;
        if !names.insert(name) {
            return Err("Duplicate command".into());
        }
        if c.get("concurrency")
            .is_some_and(|v| v != "serialize" && v != "reject-while-busy")
        {
            return Err("Invalid concurrency policy".into());
        }
        if c.get("timeoutMs")
            .is_some_and(|v| !v.as_u64().is_some_and(|n| (1..=300000).contains(&n)))
        {
            return Err("Invalid command deadline".into());
        }
        if c.get("refreshMs")
            .is_some_and(|v| !v.is_null() && !v.as_u64().is_some_and(|n| n >= 500))
        {
            return Err("Invalid refresh interval".into());
        }
    }
    super::launcher::validate(manifest)?;
    if let Some(paths) = manifest.get("migrationPaths") {
        let paths = paths.as_object().ok_or("Invalid migration paths")?;
        if paths.len() > 32 {
            return Err("Too many migration paths".into());
        }
        for (key, path) in paths {
            component(key)?;
            relative(path.as_str().ok_or("Invalid migration path")?)?;
        }
    }
    if let Some(probe) = manifest.get("startupProbe") {
        component(probe["key"].as_str().ok_or("Startup probe requires key")?)?;
        let paths = probe["migrationPaths"]
            .as_array()
            .ok_or("Startup probe requires migration path keys")?;
        if paths.is_empty()
            || paths.iter().any(|p| {
                p.as_str()
                    .is_none_or(|k| manifest["migrationPaths"].get(k).is_none())
            })
        {
            return Err("Startup probe references undeclared migration path".into());
        }
        if !caps
            .iter()
            .any(|v| v == "background" || v == "background.retain" || v == "host.background.retain")
        {
            return Err("Startup probe requires background capability".into());
        }
    }
    Ok(())
}
}
#[test] fn actual_registry_before_after_resource_root_and_aliases(){
 let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("qa/scratch");std::fs::create_dir_all(&root).unwrap();let temp=tempfile::tempdir_in(root).unwrap();let package=temp.path().join("versions/v1");std::fs::create_dir_all(package.join("resources")).unwrap();std::fs::write(package.join("resources/extension_iconv1.png"),b"synthetic icon path probe").unwrap();
 assert_eq!(before::probe(temp.path(),&package),(false,false,true,true));assert_eq!(after::probe(temp.path(),&package),(true,true,true,true));
}
#[test] fn actual_runtime_manifest_validation(){
 let manifest:serde_json::Value=serde_json::from_str(include_str!("../extension.json")).unwrap();assert!(runtime::validate(&manifest).is_ok());
 for (k,v) in [("schemaVersion",serde_json::json!(2)),("id",serde_json::json!("com.tezbar.other")),("capabilities",serde_json::json!(["core.any"])),("execution",serde_json::json!("in-process"))]{let mut bad=manifest.clone();bad[k]=v;assert!(runtime::validate(&bad).is_err())}
 for (k,v) in [("refreshMs",serde_json::json!(1)),("timeoutMs",serde_json::json!(300001)),("concurrency",serde_json::json!("anything"))]{let mut bad=manifest.clone();bad["commands"][0][k]=v;assert!(runtime::validate(&bad).is_err())}
}
