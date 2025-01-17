use self_update::self_replace;
use self_update::update::Release;
use semver::Version;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::{env, fs, io};
use zip::ZipArchive;

const REPO_OWNER: &str = "hacknus";
const REPO_NAME: &str = "serial-monitor-rust";
const MACOS_APP_NAME: &str = "Serial Monitor.app";

pub fn restart_application() {
    // Get the current executable path
    let current_exe = std::env::current_exe().expect("Failed to get current executable path");

    // Launch a new instance of the application
    let _ = Command::new(current_exe).spawn();
}

fn extract_zip(tmp_archive_path: &Path, tmp_archive_dir: &Path) -> io::Result<()> {
    // Open the zip file
    let file = File::open(tmp_archive_path)?;
    let mut archive = ZipArchive::new(file)?;

    // Iterate through the entries in the zip file
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = Path::new(tmp_archive_dir).join(file.name());

        if file.is_dir() {
            // Create directories
            std::fs::create_dir_all(&outpath)?;
        } else {
            // If the parent directory doesn't exist, create it
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)?;
                }
            }

            // Write the file to disk
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }

        // Set file permissions if needed
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
            }
        }
    }

    Ok(())
}

fn copy_dir(src: &Path, dest: &Path, binary_name: &str) -> io::Result<()> {
    // Ensure the destination directory exists
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    // Iterate through entries in the source directory
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if path.is_dir() {
            // Recursively copy subdirectories
            copy_dir(&path, &dest_path, binary_name)?;
        } else if let Some(file_name) = path.file_name() {
            if file_name != binary_name {
                // Copy files except for the binary
                fs::copy(&path, &dest_path)?;
            }
        }
    }

    Ok(())
}

pub fn check_update() -> Option<Release> {
    if let Ok(builder) = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
    {
        if let Ok(releases) = builder.fetch() {
            let current_version = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
            return releases
                .iter()
                .filter_map(|release| {
                    let release_version_str = release
                        .version
                        .strip_prefix("v")
                        .unwrap_or(&release.version);
                    Version::parse(release_version_str)
                        .ok()
                        .map(|parsed_version| (parsed_version, release))
                })
                .filter(|(parsed_version, _)| parsed_version > &current_version) // Compare versions
                .max_by(|(a, _), (b, _)| a.cmp(b)) // Find the max version
                .map(|(_, release)| release.clone()); // Return the release
        }
    }
    None
}

pub fn update(release: Release) -> Result<(), Box<dyn std::error::Error>> {
    let target_asset = if cfg!(target_os = "windows") {
        release
            .asset_for(self_update::get_target(), Some("exe"))
            .unwrap()
    } else if cfg!(target_os = "linux") {
        release
            .asset_for(self_update::get_target(), Some("bin"))
            .unwrap()
    } else {
        release.asset_for(self_update::get_target(), None).unwrap()
    };
    let tmp_archive_dir = tempfile::TempDir::new()?;
    let tmp_archive_path = tmp_archive_dir.path().join(&target_asset.name);
    let tmp_archive = fs::File::create(&tmp_archive_path)?;

    self_update::Download::from_url(&target_asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(&tmp_archive)?;

    extract_zip(&tmp_archive_path, tmp_archive_dir.path())?;
    let new_exe = if cfg!(target_os = "windows") {
        let binary = env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        tmp_archive_dir.path().join(binary)
    } else if cfg!(target_os = "macos") {
        let binary = env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let app_dir = env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();

        let app_name = app_dir
            .clone()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let _ = copy_dir(&tmp_archive_dir.path().join(&app_name), &app_dir, &binary);

        // MACOS_APP_NAME either needs to be hardcoded or extracted from the downloaded and
        // extracted archive, but we cannot just assume that the parent directory of the
        // currently running executable is equal to the app name - this is especially not
        // the case if we run the code with `cargo run`.
        tmp_archive_dir
            .path()
            .join(format!("{}/Contents/MacOS/{}", MACOS_APP_NAME, binary))
    } else if cfg!(target_os = "linux") {
        let binary = env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        tmp_archive_dir.path().join(binary)
    } else {
        panic!("Running on an unsupported OS");
    };

    self_replace::self_replace(new_exe)?;
    Ok(())
}
