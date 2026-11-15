use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::game::Platform;

const TOU_REPO: &str = "AU-Avengers/TOU-Mira";
const MARKER_FILE: &str = "BepInEx/ModdedAmongUs.txt";
const ARCHIVE_DIR: &str = "ModdedAmongUs";
const CHOICE_FILE: &str = "ModdedAmongUs/choice.txt";
const MOD_ENTRIES: [&str; 7] = [
    "winhttp.dll",
    "doorstop_config.ini",
    ".doorstop_version",
    "BepInEx",
    "dotnet",
    "CorsacCosmetics",
    "steam_appid.txt",
];
const UNWANTED: [&str; 3] = [
    "BepInEx/patchers/BepInEx.SplashScreen",
    "BepInEx/plugins/LevelImposter.dll",
    "BepInEx/plugins/LevelImposter",
];
const SHARED_CACHE: [&str; 2] = ["BepInEx/interop", "BepInEx/unity-libs"];
const CACHE_HASH: &str = "BepInEx/interop/assembly-hash.txt";
const REQUIRED: [&str; 3] = [
    "winhttp.dll",
    "BepInEx/plugins/MiraAPI.dll",
    "BepInEx/plugins/TownOfUsMira.dll",
];
const USER_AGENT: &str = "ModdedAmongUsInstaller/2.0";

pub const LATEST: &str = "Latest";

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModSet {
    pub tou_tag: String,
    pub tou_asset: String,
    pub tou_url: String,
    pub notes_url: String,
    pub prerelease: bool,
    pub fits: bool,
    pub too_old: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub enabled: bool,
    pub version: String,
}

/// List every TOU-Mira release that has a Windows zip, newest first.
///
/// @param platform: Store the game was installed from.
/// @param arch: Game exe architecture ("x86" or "x64").
/// @param game_date: Game version date, if known.
/// @return: Installable mod sets.
pub fn list_versions(
    platform: Platform,
    arch: &str,
    game_date: Option<(u32, u32, u32)>,
) -> Result<Vec<ModSet>, String> {
    let sets = pick_tou(&fetch_releases(TOU_REPO)?, platform, arch, game_date);
    if sets.is_empty() {
        return Err("No TOU-Mira release has a Windows zip.".into());
    }
    Ok(sets)
}

/// Pick the best Windows zip of every release: same architecture first, then same store.
///
/// @param releases: Releases newest first, as GitHub returns them.
/// @param platform: Store the game was installed from.
/// @param arch: Game exe architecture ("x86" or "x64").
/// @param game_date: Game version date; releases published before it are marked too old.
/// @return: One mod set per release that has a Windows zip, newest first.
pub fn pick_tou(
    releases: &[Release],
    platform: Platform,
    arch: &str,
    game_date: Option<(u32, u32, u32)>,
) -> Vec<ModSet> {
    let store = platform.name().to_lowercase();
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let asset = r
                .assets
                .iter()
                .filter(|a| {
                    let name = a.name.to_lowercase();
                    name.ends_with(".zip") && !name.contains("macos") && !name.contains("linux")
                })
                .max_by_key(|a| {
                    let name = a.name.to_lowercase();
                    (name.contains(arch), name.contains(&store))
                })?;
            Some(ModSet {
                tou_tag: r.tag_name.clone(),
                tou_asset: asset.name.clone(),
                tou_url: asset.browser_download_url.clone(),
                notes_url: r.html_url.clone(),
                prerelease: r.prerelease,
                fits: asset.name.to_lowercase().contains(arch),
                too_old: game_date
                    .is_some_and(|g| release_date(&r.published_at).map_or(true, |d| d < g)),
            })
        })
        .collect()
}

/// Read the date part of a GitHub timestamp like "2026-10-19T00:51:13Z".
///
/// @param stamp: Timestamp text.
/// @return: (year, month, day), or None if it cannot be read.
pub fn release_date(stamp: &str) -> Option<(u32, u32, u32)> {
    let mut parts = stamp.get(..10)?.split('-').map(|p| p.parse().ok());
    Some((parts.next()??, parts.next()??, parts.next()??))
}

/// Resolve "Latest" to the newest full release built for the game's architecture and version.
///
/// @param versions: Versions from list_versions.
/// @return: That version, if any.
pub fn latest(versions: &[ModSet]) -> Option<&ModSet> {
    versions
        .iter()
        .find(|v| v.fits && !v.too_old && !v.prerelease)
}

/// Read which TOU-Mira version is active in the game folder.
///
/// @param game_dir: Among Us folder.
/// @return: The active version, or None if no mod from this tool is active.
pub fn installed_version(game_dir: &Path) -> Option<String> {
    fs::read_to_string(game_dir.join(MARKER_FILE))
        .ok()
        .and_then(|s| s.lines().next().map(|l| l.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// List versions kept in the archive folder.
///
/// @param game_dir: Among Us folder.
/// @return: Archived version tags.
pub fn archived_versions(game_dir: &Path) -> Vec<String> {
    fs::read_dir(game_dir.join(ARCHIVE_DIR))
        .map(|dir| {
            dir.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// Read the last choice saved in this game folder.
///
/// @param game_dir: Among Us folder.
/// @return: The saved choice, or mods on with the latest version.
pub fn load_choice(game_dir: &Path) -> Choice {
    let text = fs::read_to_string(game_dir.join(CHOICE_FILE)).unwrap_or_default();
    let mut lines = text.lines();
    Choice {
        enabled: lines.next() != Some("off"),
        version: lines
            .next()
            .filter(|v| !v.is_empty())
            .unwrap_or(LATEST)
            .to_string(),
    }
}

/// Save the choice so the next start uses it.
///
/// @param game_dir: Among Us folder.
/// @param choice: Choice to save.
/// @return: Ok once written.
pub fn save_choice(game_dir: &Path, choice: &Choice) -> Result<(), String> {
    fs::create_dir_all(game_dir.join(ARCHIVE_DIR)).map_err(|e| e.to_string())?;
    let state = if choice.enabled { "on" } else { "off" };
    fs::write(
        game_dir.join(CHOICE_FILE),
        format!("{state}\n{}", choice.version),
    )
    .map_err(|e| e.to_string())
}

/// Make the game folder match the choice: archive the active mod, then restore or download the wanted one.
///
/// @param game_dir: Among Us folder.
/// @param wanted: Version tag to activate, or None to run the game without mods.
/// @param versions: Downloadable versions, used when the wanted one is not archived.
/// @param progress: Called with a status line and a 0..1 fraction.
/// @return: Ok once the folder matches.
pub fn switch_to(
    game_dir: &Path,
    wanted: Option<&str>,
    versions: &[ModSet],
    progress: &dyn Fn(String, f32),
) -> Result<(), String> {
    if installed_version(game_dir).as_deref() != wanted {
        archive_active(game_dir)?;
    }
    let Some(tag) = wanted else {
        return Ok(());
    };
    if installed_version(game_dir).as_deref() != Some(tag) && !restore(game_dir, tag)? {
        let set = versions.iter().find(|v| v.tou_tag == tag).ok_or_else(|| {
            format!("Version {tag} is not downloaded and could not be found online.")
        })?;
        install(game_dir, set, progress)?;
    }
    remove_unwanted(game_dir)?;
    let missing: Vec<&str> = REQUIRED
        .into_iter()
        .filter(|f| !game_dir.join(f).exists())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Version {tag} is missing {}. Pick it again to download it fresh.",
        missing.join(", ")
    ))
}

/// Delete LevelImposter and the BepInEx loading splash, whose .NET Framework GUI makes Windows ask to install .NET Framework.
///
/// @param game_dir: Among Us folder.
/// @return: Ok once those files are gone.
pub fn remove_unwanted(game_dir: &Path) -> Result<(), String> {
    for name in UNWANTED {
        let path = game_dir.join(name);
        let result = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else if path.exists() {
            fs::remove_file(&path)
        } else {
            Ok(())
        };
        result.map_err(|e| format!("Could not remove {name}: {e}"))?;
    }
    Ok(())
}

/// Download and install a mod set into a game folder that has no active mod.
///
/// @param game_dir: Among Us folder.
/// @param set: Mod set to install.
/// @param progress: Called with a status line and a 0..1 fraction.
/// @return: Ok once every file is written.
pub fn install(
    game_dir: &Path,
    set: &ModSet,
    progress: &dyn Fn(String, f32),
) -> Result<(), String> {
    let tou_zip = download(&set.tou_url, &format!("TOU-Mira {}", set.tou_tag), progress)?;
    progress("Installing files...".into(), 1.0);
    let entries = extract_zip(&tou_zip, game_dir)?;
    reuse_cache(game_dir)?;
    let marker = std::iter::once(set.tou_tag.clone())
        .chain(entries)
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    fs::write(game_dir.join(MARKER_FILE), marker).map_err(|e| e.to_string())
}

/// Copy the generated BepInEx interop cache from a saved version so the first start skips the slow rebuild.
///
/// @param game_dir: Among Us folder with a freshly extracted version active.
/// @return: Ok once copied, or when there is nothing to copy.
pub fn reuse_cache(game_dir: &Path) -> Result<(), String> {
    if game_dir.join(CACHE_HASH).exists() {
        return Ok(());
    }
    let Some(donor) = archived_versions(game_dir)
        .into_iter()
        .map(|tag| game_dir.join(ARCHIVE_DIR).join(tag))
        .find(|dir| dir.join(CACHE_HASH).exists())
    else {
        return Ok(());
    };
    for name in SHARED_CACHE {
        copy_dir(&donor.join(name), &game_dir.join(name))
            .map_err(|e| format!("Could not copy {name}: {e}"))?;
    }
    Ok(())
}

/// Move the active mod files into the archive under their version.
///
/// @param game_dir: Among Us folder.
/// @return: Ok once the game folder has no mod files from this tool.
pub fn archive_active(game_dir: &Path) -> Result<(), String> {
    let Some(tag) = installed_version(game_dir) else {
        return Ok(());
    };
    let marker = fs::read_to_string(game_dir.join(MARKER_FILE)).unwrap_or_default();
    let mut entries: Vec<String> = MOD_ENTRIES.iter().map(|e| e.to_string()).collect();
    entries.extend(
        marker
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|e| !e.is_empty() && !entries.iter().any(|m| m == e))
            .map(String::from)
            .collect::<Vec<_>>(),
    );
    let dest = game_dir.join(ARCHIVE_DIR).join(&tag);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| format!("Could not clear archive {tag}: {e}"))?;
    }
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    move_entries(game_dir, &dest, &entries)
}

/// Move an archived version back into the game folder.
///
/// @param game_dir: Among Us folder.
/// @param tag: Version tag.
/// @return: True if it was archived and is now active.
pub fn restore(game_dir: &Path, tag: &str) -> Result<bool, String> {
    let src = game_dir.join(ARCHIVE_DIR).join(tag);
    if !src.is_dir() {
        return Ok(false);
    }
    let marker = fs::read_to_string(src.join(MARKER_FILE)).unwrap_or_default();
    if marker.lines().count() < 2 {
        fs::remove_dir_all(&src).map_err(|e| format!("Could not clear old archive {tag}: {e}"))?;
        return Ok(false);
    }
    let entries: Vec<String> = fs::read_dir(&src)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    move_entries(&src, game_dir, &entries)?;
    fs::remove_dir_all(&src).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Extract a zip into a folder, dropping a single wrapping top folder and skipping unsafe paths.
///
/// @param data: Zip bytes.
/// @param dest: Folder to extract into.
/// @return: Top level file and folder names that were written.
pub fn extract_zip(data: &[u8], dest: &Path) -> Result<Vec<String>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| format!("Bad zip: {e}"))?;
    let names: Vec<PathBuf> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok()?.enclosed_name())
        .collect();
    let wrapper = wrapping_folder(&names);
    let mut top: Vec<String> = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let rel = wrapper
            .as_ref()
            .and_then(|w| name.strip_prefix(w).ok())
            .unwrap_or(&name);
        if rel.as_os_str().is_empty() {
            continue;
        }
        let first = rel
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        if !top.contains(&first) {
            top.push(first);
        }
        let target = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        fs::write(&target, buf)
            .map_err(|e| format!("Could not write {}: {e}", target.display()))?;
    }
    Ok(top)
}

/// Find a single top folder that every zip entry sits inside.
///
/// @param names: Entry paths.
/// @return: That folder, or None if entries are not all wrapped.
pub fn wrapping_folder(names: &[PathBuf]) -> Option<PathBuf> {
    let first = PathBuf::from(names.first()?.components().next()?.as_os_str());
    let wraps_all = names
        .iter()
        .all(|n| n.starts_with(&first) && (n.components().count() > 1 || n == &first));
    (wraps_all && names.iter().any(|n| n.components().count() > 1)).then_some(first)
}

/// Copy a folder and everything in it.
///
/// @param from: Source folder.
/// @param to: Destination folder, created if missing.
/// @return: Ok once every file is copied.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Move named files and folders from one folder to another on the same drive.
///
/// @param from: Source folder.
/// @param to: Destination folder.
/// @param entries: Top level names to move; missing ones are skipped.
/// @return: Ok once every present entry is moved.
fn move_entries(from: &Path, to: &Path, entries: &[String]) -> Result<(), String> {
    for name in entries {
        let src = from.join(name);
        if src.exists() {
            fs::rename(&src, to.join(name)).map_err(|e| {
                format!("Could not move {name}: {e}. Close Among Us and try again.")
            })?;
        }
    }
    Ok(())
}

/// Fetch the release list of a GitHub repo, newest first.
///
/// @param repo: "owner/name".
/// @return: Parsed releases.
fn fetch_releases(repo: &str) -> Result<Vec<Release>, String> {
    ureq::get(&format!(
        "https://api.github.com/repos/{repo}/releases?per_page=100"
    ))
    .set("User-Agent", USER_AGENT)
    .set("Accept", "application/vnd.github+json")
    .call()
    .map_err(|e| format!("Could not reach GitHub for {repo}: {e}"))?
    .into_json()
    .map_err(|e| format!("Bad release data from {repo}: {e}"))
}

/// Download a file into memory while reporting progress.
///
/// @param url: Direct download URL.
/// @param label: Name shown in the status line.
/// @param progress: Called with a status line and a 0..1 fraction.
/// @return: File bytes.
fn download(url: &str, label: &str, progress: &dyn Fn(String, f32)) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("Download of {label} failed: {e}"))?;
    let total: usize = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut data = Vec::with_capacity(total);
    let mut chunk = vec![0u8; 256 * 1024];
    loop {
        let read = reader
            .read(&mut chunk)
            .map_err(|e| format!("Download of {label} failed: {e}"))?;
        if read == 0 {
            return Ok(data);
        }
        data.extend_from_slice(&chunk[..read]);
        let fraction = if total > 0 {
            data.len() as f32 / total as f32
        } else {
            0.0
        };
        progress(
            format!("Downloading {label}: {} MB", data.len() / (1024 * 1024)),
            fraction,
        );
    }
}
