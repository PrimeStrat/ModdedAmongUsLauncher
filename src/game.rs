use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const STEAM_APP_ID: &str = "945360";
const EPIC_DISPLAY_NAME: &str = "Among Us";
const GAME_EXE: &str = "Among Us.exe";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Steam,
    Epic,
}

#[derive(Clone, Debug)]
pub struct GameInstall {
    pub platform: Platform,
    pub dir: PathBuf,
    pub launch_uri: String,
}

impl Platform {
    /// Name shown in the UI and matched against release asset names.
    ///
    /// @return: "Steam" or "Epic".
    pub fn name(self) -> &'static str {
        match self {
            Platform::Steam => "Steam",
            Platform::Epic => "Epic",
        }
    }
}

impl GameInstall {
    /// Read the game exe PE header to get its CPU architecture.
    ///
    /// @return: "x64", "x86", or "???" if the exe cannot be read.
    pub fn arch(&self) -> &'static str {
        pe_arch(&self.dir.join(GAME_EXE))
    }

    /// Start the game through its store so ownership and login are handled by the store.
    ///
    /// @return: Ok if the launch command was started.
    pub fn launch(&self) -> Result<(), String> {
        Command::new("explorer.exe")
            .arg(&self.launch_uri)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not launch {}: {e}", self.platform.name()))
    }
}

/// Find every Steam and Epic install of Among Us on this PC.
///
/// @return: All detected installs, Steam first.
pub fn detect_installs() -> Vec<GameInstall> {
    find_steam().into_iter().chain(find_epic()).collect()
}

/// Read the machine type from a PE file header.
///
/// @param exe: Path to a Windows executable.
/// @return: "x64", "x86", or "???".
pub fn pe_arch(exe: &Path) -> &'static str {
    let Ok(data) = fs::read(exe) else {
        return "???";
    };
    let read_u32 = |at: usize| {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let Some(pe) = read_u32(0x3C).map(|v| v as usize) else {
        return "???";
    };
    match data
        .get(pe + 4..pe + 6)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
    {
        Some(0x8664) => "x64",
        Some(0x014C) => "x86",
        _ => "???",
    }
}

/// Pull every quoted value for a key out of a Steam VDF/ACF text file.
///
/// @param text: File contents.
/// @param key: Key name without quotes.
/// @return: Values in file order, with escaped backslashes undone.
pub fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let quoted = format!("\"{key}\"");
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix(&quoted)?;
            let value = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some(value.replace("\\\\", "\\"))
        })
        .collect()
}

/// Find Among Us in any Steam library listed in libraryfolders.vdf.
///
/// @return: The Steam install, if found.
fn find_steam() -> Option<GameInstall> {
    let steam: String = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Valve\\Steam")
        .ok()?
        .get_value("SteamPath")
        .ok()?;
    let steamapps = PathBuf::from(steam).join("steamapps");
    let mut libraries = vdf_values(
        &fs::read_to_string(steamapps.join("libraryfolders.vdf")).unwrap_or_default(),
        "path",
    )
    .into_iter()
    .map(|p| PathBuf::from(p).join("steamapps"))
    .collect::<Vec<_>>();
    libraries.push(steamapps);
    libraries.into_iter().find_map(|lib| {
        let manifest =
            fs::read_to_string(lib.join(format!("appmanifest_{STEAM_APP_ID}.acf"))).ok()?;
        let dir = lib
            .join("common")
            .join(vdf_values(&manifest, "installdir").first()?);
        dir.join(GAME_EXE).exists().then(|| GameInstall {
            platform: Platform::Steam,
            dir,
            launch_uri: format!("steam://rungameid/{STEAM_APP_ID}"),
        })
    })
}

/// Find Among Us in the Epic Games Launcher install manifests.
///
/// @return: The Epic install, if found.
fn find_epic() -> Option<GameInstall> {
    let manifests = PathBuf::from(std::env::var("PROGRAMDATA").ok()?)
        .join("Epic\\EpicGamesLauncher\\Data\\Manifests");
    fs::read_dir(manifests).ok()?.flatten().find_map(|entry| {
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(entry.path()).ok()?).ok()?;
        if json["DisplayName"].as_str()? != EPIC_DISPLAY_NAME {
            return None;
        }
        let dir = PathBuf::from(json["InstallLocation"].as_str()?);
        dir.join(GAME_EXE).exists().then(|| GameInstall {
            platform: Platform::Epic,
            dir,
            launch_uri: format!(
                "com.epicgames.launcher://apps/{}?action=launch&silent=true",
                json["AppName"].as_str().unwrap_or_default()
            ),
        })
    })
}
