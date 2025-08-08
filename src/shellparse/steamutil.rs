/*
* Copyright © 2025 Alessandro Balducci
*
* This file is part of Desktop File Editor.
* Desktop File Editor is free software: you can redistribute it and/or modify it under the terms of the 
* GNU General Public License as published by the Free Software Foundation, 
* either version 3 of the License, or (at your option) any later version.
* Desktop File Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
* without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
* See the GNU General Public License for more details.
* You should have received a copy of the GNU General Public License along with Desktop File Editor. If not, see <https://www.gnu.org/licenses/>.
*/

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use const_format::concatcp;
use once_cell::sync::Lazy;
use serde::Deserialize;

/// Relative to home directory
const STEAM_DIR: &str = ".steam";
/// Relative to home directory
const DEFAULT_STEAMAPPS_DIR: &str = concatcp!(STEAM_DIR, "/steam/steamapps");
/// Relative to home directory
const LIBRARYFOLDERS_VDF: &str = concatcp!(DEFAULT_STEAMAPPS_DIR, "/libraryfolders.vdf");

// Expand home relative paths into absolute paths
fn expand_path(path: impl AsRef<Path>) -> PathBuf {
    let homedir = env::home_dir().expect("Could not find home directory");
    Path::new(&homedir).join(path)
}

fn library_folders_path() -> PathBuf {
    expand_path(LIBRARYFOLDERS_VDF)
}

fn app_manifest_path(steamapps_path: impl AsRef<Path>, app_id: u64) -> PathBuf {
    expand_path(
        steamapps_path
            .as_ref()
            .join(format!("appmanifest_{app_id}.acf")),
    )
}

#[derive(Debug, Deserialize, PartialEq)]
struct LibraryFolder {
    path: String,
    label: String,
    #[serde(rename = "contentid")]
    content_id: u64,
    #[serde(rename = "totalsize")]
    total_size: u64,
    update_clean_bytes_tally: u64,
    time_last_update_verified: u64,
    apps: BTreeMap<u64, u64>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct LibraryFolders {
    #[serde(rename = "libraryfolders")]
    folders: Vec<LibraryFolder>,
}

// #[derive(Debug, Deserialize, PartialEq)]
// struct AppManifest {
//     #[serde(rename = "AppState")]
//     state: AppState,
// }
//
// #[derive(Debug, Deserialize, PartialEq)]
// struct AppState {
//     #[serde(rename = "appid")]
//     app_id: u64,
//     #[serde(rename = "Universe")]
//     universe: u64,
//     name: String,
//     #[serde(rename = "StateFlags")]
//     state_flags: u64,
//     #[serde(rename = "installdir")]
//     install_dir: String,
//     #[serde(rename = "LastUpdated")]
//     last_updated: u64,
//     #[serde(rename = "LastPlayed")]
//     last_played: Option<u64>,
//     #[serde(rename = "SizeOnDisk")]
//     size_on_disk: u64,
//     #[serde(rename = "StagingSize")]
//     staging_size: u64,
//     #[serde(rename = "buildid")]
//     build_id: u64,
//     #[serde(rename = "LastOwner")]
//     last_owner: u64,
//     #[serde(rename = "DownloadType")]
//     download_type: Option<u64>,
//     #[serde(rename = "UpdateResult")]
//     update_result: Option<u64>,
//     #[serde(rename = "BytesToDownload")]
//     bytes_to_download: Option<u64>,
//     #[serde(rename = "BytesDownloaded")]
//     bytes_downloaded: Option<u64>,
//     #[serde(rename = "BytesToStage")]
//     bytes_to_stage: Option<u64>,
//     #[serde(rename = "BytesStaged")]
//     bytes_staged: Option<u64>,
//     #[serde(rename = "TargetBuildID")]
//     target_build_id: Option<u64>,
//     #[serde(rename = "AutoUpdateBehavior")]
//     auto_update_behavior: u64,
//     #[serde(rename = "AllowOtherDownloadsWhileRunning")]
//     allow_other_downloads_while_running: bool,
//     #[serde(rename = "ScheduledAutoUpdate")]
//     scheduled_autoupdate: u64,
//     #[serde(rename = "InstalledDepots")]
//     installed_depots: BTreeMap<u64, InstalledDepot>,
//     #[serde(rename = "SharedDepots")]
//     shared_depots: Option<BTreeMap<u64, u64>>,
//     #[serde(rename = "StagedDepots")]
//     staged_depots: Option<BTreeMap<u64, StagedDepot>>,
//     #[serde(rename = "UserConfig")]
//     user_config: BTreeMap<String, String>,
//     #[serde(rename = "MountedConfig")]
//     mounted_config: BTreeMap<String, String>,
// }
//
// #[derive(Debug, Deserialize, PartialEq)]
// struct InstalledDepot {
//     manifest: u64,
//     size: u64,
// }
//
// #[derive(Debug, Deserialize, PartialEq)]
// struct StagedDepot {
//     manifest: u64,
//     size: u64,
//     dlcappid: u64,
// }

// FIXME: this will not be updated if the file changes, but it shouldn't matter much unless the
// user creates or deletes steam libraries
static LIBRARY_FOLDERS: Lazy<Option<LibraryFolders>> = Lazy::new(|| {
    let data = fs::read_to_string(library_folders_path()).ok()?;
    vdf_reader::from_str(&data).ok()
});

fn find_steamapps_path_for_app(app_id: u64) -> Option<PathBuf> {
    LIBRARY_FOLDERS.as_ref().and_then(|library_folders| {
        for folder in &library_folders.folders {
            if folder
                .apps
                .keys()
                .any(|&folder_app_id| app_id == folder_app_id)
            {
                return Some(Path::new(&folder.path).join("steamapps"));
            }
        }
        None
    })
}

pub fn is_app_installed(app_id: u64) -> bool {
    let steamapps_path = match find_steamapps_path_for_app(app_id) {
        Some(steamapps_path) => steamapps_path,
        None => return false,
    };

    let app_manifest_path = app_manifest_path(steamapps_path, app_id);
    if !app_manifest_path.exists() {
        return false;
    }

    app_manifest_path.exists()

    // TODO: Maybe cache installed appids so we don't need to read the file every time?
    // Otherwise we could avoid reading the file altogether and just rely on the existance of the
    // appmanifest itself
    // let data = match fs::read_to_string(&app_manifest_path) {
    //     Ok(data) => data,
    //     Err(e) => {
    //         println!(
    //             "Failed to read app manifest '{}': {}",
    //             app_manifest_path.to_string_lossy(),
    //             e
    //         );
    //         return false;
    //     }
    // };
    //
    // let app_manifest: AppManifest = match vdf_reader::from_str(&data) {
    //     Ok(app_manifest) => app_manifest,
    //     Err(e) => {
    //         println!(
    //             "Failed to parse app manifest '{}': {}",
    //             app_manifest_path.to_string_lossy(),
    //             e
    //         );
    //         return false;
    //     }
    // };
    //
    // app_manifest.state.app_id == app_id
}
