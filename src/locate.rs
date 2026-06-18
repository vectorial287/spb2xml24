//! Best effort discovery of the Microsoft Flight Simulator 2024 propdefs
//! directory across the common install layouts (Microsoft Store / Xbox, Steam,
//! and relocated installs recorded in `UserCfg.opt`).
//!
//! Each candidate path is confirmed by the presence of `propbase.xml`, the
//! foundational definition file, so a wrong or empty directory is never
//! returned. Discovery only finds anything on Windows; elsewhere the caller
//! must supply the directory explicitly.

use std::path::{Path, PathBuf};

const GAME: &str = "Microsoft Flight Simulator 2024";
const CONTENT_PROPDEFS: &str = "Content/Propdefs/1.0/Common";
const MARKER: &str = "propbase.xml";

/// Return the first propdefs directory found in a known MSFS 2024 location,
/// or `None` if none could be located.
pub fn find_propdefs() -> Option<PathBuf> {
    candidates().into_iter().find(|dir| is_propdefs(dir))
}

/// A directory is treated as a propdefs folder when it contains `propbase.xml`.
pub fn is_propdefs(dir: &Path) -> bool {
    dir.join(MARKER).is_file()
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // Microsoft Store / Xbox app: <drive>:\XboxGames\<game>\Content\Propdefs\...
    for drive in fixed_drive_roots() {
        out.push(drive.join("XboxGames").join(GAME).join(CONTENT_PROPDEFS));
    }

    // Steam: <library>\steamapps\common\<game>\Content\Propdefs\...
    for library in steam_libraries() {
        out.push(
            library
                .join("steamapps")
                .join("common")
                .join(GAME)
                .join(CONTENT_PROPDEFS),
        );
    }

    // Relocated or community installs recorded in UserCfg.opt.
    for base in installed_packages_paths() {
        out.push(base.join("Official/OneStore/fs-base-propdefs/Propdefs/1.0/Common"));
        out.push(base.join("Official/Steam/fs-base-propdefs/Propdefs/1.0/Common"));
        out.push(base.join(CONTENT_PROPDEFS));
        out.push(base.join("Propdefs/1.0/Common"));
    }

    out
}

/// Read `InstalledPackagesPath` from the Store and Steam `UserCfg.opt` files.
fn installed_packages_paths() -> Vec<PathBuf> {
    let mut configs = Vec::new();
    if let Some(local) = env_path("LOCALAPPDATA") {
        configs
            .push(local.join(r"Packages\Microsoft.Limitless_8wekyb3d8bbwe\LocalCache\UserCfg.opt"));
    }
    if let Some(roaming) = env_path("APPDATA") {
        configs.push(roaming.join(GAME).join("UserCfg.opt"));
    }

    configs
        .into_iter()
        .filter_map(|cfg| std::fs::read_to_string(cfg).ok())
        .filter_map(|text| parse_installed_packages_path(&text))
        .map(PathBuf::from)
        .collect()
}

/// Collect Steam library roots from the default Steam install and any extra
/// libraries listed in `libraryfolders.vdf`.
fn steam_libraries() -> Vec<PathBuf> {
    let mut steam_dirs = Vec::new();
    for var in ["ProgramFiles(x86)", "ProgramW6432", "ProgramFiles"] {
        if let Some(dir) = env_path(var) {
            steam_dirs.push(dir.join("Steam"));
        }
    }

    let mut roots = Vec::new();
    for steam in steam_dirs {
        if steam.join("steamapps").is_dir() {
            roots.push(steam.clone());
        }
        if let Ok(text) = std::fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
            roots.extend(parse_library_paths(&text).into_iter().map(PathBuf::from));
        }
    }
    roots
}

#[cfg(windows)]
fn fixed_drive_roots() -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
        .filter(|root| root.is_dir())
        .collect()
}

#[cfg(not(windows))]
fn fixed_drive_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn parse_installed_packages_path(config: &str) -> Option<String> {
    config
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("InstalledPackagesPath"))
        .and_then(quoted_value)
}

fn parse_library_paths(vdf: &str) -> Vec<String> {
    vdf.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("\"path\""))
        .filter_map(quoted_value)
        .collect()
}

/// Extract the first double quoted substring, unescaping the doubled
/// backslashes that the VDF and config formats use.
fn quoted_value(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    Some(text[start..end].replace("\\\\", "\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_installed_packages_path() {
        let cfg = "Fov 50\nInstalledPackagesPath \"D:\\\\MSFS 2024\\\\Packages\"\nState 1\n";
        assert_eq!(
            parse_installed_packages_path(cfg).as_deref(),
            Some(r"D:\MSFS 2024\Packages")
        );
    }

    #[test]
    fn reads_steam_library_paths() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"E:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(
            parse_library_paths(vdf),
            vec![
                r"C:\Program Files (x86)\Steam".to_string(),
                r"E:\SteamLibrary".to_string(),
            ]
        );
    }
}
