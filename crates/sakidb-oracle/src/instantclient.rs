use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{info, warn};
use dirs::data_dir;
use sakidb_core::error::{Result, SakiError};

const INSTANTCLIENT_VERSION: &str = "21.13.0.0.0";
const INSTANTCLIENT_BASE_URL: &str = "https://download.oracle.com/otn_software/mac/instantclient";

/// Ensures Oracle Instant Client is available for the current platform
/// Downloads and configures it automatically if not found
pub async fn ensure_instantclient() -> Result<()> {
    // First check if OCI_LIB_DIR is already set
    if env::var("OCI_LIB_DIR").is_ok() {
        info!("OCI_LIB_DIR already set, skipping instantclient setup");
        return Ok(());
    }

    // Check if instantclient is already available in system paths
    if is_instantclient_available() {
        info!("InstantClient already available in system paths");
        return Ok(());
    }

    // Determine platform and download appropriate version
    let platform = determine_platform()?;
    let instantclient_dir = get_local_instantclient_dir(&platform)?;
    
    if !instantclient_dir.exists() {
        info!("Downloading Oracle InstantClient for platform: {}", platform);
        download_instantclient(&platform, &instantclient_dir).await?;
    }

    // Set OCI_LIB_DIR environment variable
    // Safety: env::set_var is unsafe in multi-threaded environments (Rust 1.81+)
    // In sakidb, this is called during connection setup.
    unsafe {
        env::set_var("OCI_LIB_DIR", &instantclient_dir);
    }
    info!("Set OCI_LIB_DIR to: {}", instantclient_dir.display());
    
    // Add to library path if needed
    update_library_path(&instantclient_dir, &platform)?;
    
    if platform.starts_with("macos") {
        warn!("On macOS, DYLD_LIBRARY_PATH cannot be set at runtime after process start.");
        warn!("If connection fails, please set DYLD_LIBRARY_PATH manually and restart SakiDB:");
        warn!("export DYLD_LIBRARY_PATH={}:$DYLD_LIBRARY_PATH", instantclient_dir.display());
    }
    
    Ok(())
}

fn determine_platform() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    
    match (os, arch) {
        ("macos", "aarch64") => Ok("macos-arm64".to_string()),
        ("macos", "x86_64") => Ok("macos-x64".to_string()),
        ("linux", "x86_64") => Ok("linux-x64".to_string()),
        ("windows", "x86_64") => Ok("windows-x64".to_string()),
        _ => Err(SakiError::ConnectionFailed(format!(
            "Unsupported platform: {}-{}", os, arch
        ))),
    }
}

fn get_local_instantclient_dir(platform: &str) -> Result<PathBuf> {
    let data_dir = data_dir()
        .ok_or_else(|| SakiError::ConnectionFailed("Failed to get data directory".to_string()))?;
    
    let instantclient_dir = data_dir
        .join("sakidb")
        .join("instantclient")
        .join(platform)
        .join(format!("instantclient_{}", INSTANTCLIENT_VERSION));
    
    Ok(instantclient_dir)
}

fn is_instantclient_available() -> bool {
    Command::new("ldconfig").arg("-p").output().is_ok_and(|output| {
        String::from_utf8_lossy(&output.stdout).contains("libclntsh")
    })
}

async fn download_instantclient(platform: &str, target_dir: &PathBuf) -> Result<()> {
    // Create target directory
    fs::create_dir_all(target_dir).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create directory: {}", e)))?;

    let filename = match platform {
        "macos-arm64" => "instantclient-basic-macos.arm64.dmg",
        "macos-x64" => "instantclient-basic-macos.x64.dmg",
        "linux-x64" => "instantclient-basic-linux.x64-21.13.0.0.0dbru.zip",
        "windows-x64" => "instantclient-basic-windows.x64-21.13.0.0.0dbru.zip",
        _ => return Err(SakiError::ConnectionFailed(format!("Unsupported platform: {}", platform))),
    };

    let download_url = format!("{}/{}", INSTANTCLIENT_BASE_URL, filename);
    info!("Downloading InstantClient from: {}", download_url);

    // Download file
    let response = reqwest::get(&download_url).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to download InstantClient: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SakiError::ConnectionFailed(
            format!("Failed to download InstantClient: HTTP {}", response.status())
        ));
    }

    let bytes = response.bytes().await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read download: {}", e)))?;

    let temp_file = target_dir.join(filename);
    fs::write(&temp_file, &bytes).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to write download: {}", e)))?;

    // Extract based on file type
    if filename.ends_with(".zip") {
        extract_zip(&temp_file, target_dir).await?;
    } else if filename.ends_with(".dmg") {
        extract_dmg(&temp_file, target_dir).await?;
    }

    // Clean up downloaded file (ignore errors)
    let _ = fs::remove_file(temp_file).await;

    info!("InstantClient extracted successfully to: {}", target_dir.display());
    Ok(())
}

async fn extract_zip(zip_file: &Path, target_dir: &Path) -> Result<()> {
    let zip_path = zip_file.to_path_buf();
    let target = target_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path)
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to open zip file: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read zip archive: {}", e)))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| SakiError::ConnectionFailed(format!("Failed to get file from zip: {}", e)))?;
            
            let ename = entry.name();
            // Sanitize path to prevent Zip Slip
            let outpath = target.join(ename);
            
            if !outpath.starts_with(&target) {
                return Err(SakiError::ConnectionFailed(format!("Invalid zip entry path (possible Zip Slip): {}", ename)));
            }

            if entry.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)
                    .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create directory: {}", e)))?;
            } else {
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)
                        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create parent directory: {}", e)))?;
                }
                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create file: {}", e)))?;
                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| SakiError::ConnectionFailed(format!("Failed to write file: {}", e)))?;
            }
        }
        Ok::<(), SakiError>(())
    }).await.map_err(|e| SakiError::ConnectionFailed(format!("Zip extraction task failed: {}", e)))??;
    Ok(())
}

async fn extract_dmg(dmg_file: &Path, target_dir: &Path) -> Result<()> {
    // For macOS, we need to mount the DMG and copy files
    // This is a simplified approach - in production, you'd want more robust handling
    
    let mount_output = Command::new("hdiutil")
        .args(["attach", "-quiet", "-nobrowse", dmg_file.to_str().unwrap()])
        .output()
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to mount DMG: {}", e)))?;

    if !mount_output.status.success() {
        return Err(SakiError::ConnectionFailed(
            format!("Failed to mount DMG: {}", String::from_utf8_lossy(&mount_output.stderr))
        ));
    }

    // Parse mount point from output
    let output_str = String::from_utf8_lossy(&mount_output.stdout);
    let mount_point = output_str
        .lines()
        .find(|line| line.contains("/Volumes/"))
        .and_then(|line| line.split_whitespace().last())
        .ok_or_else(|| SakiError::ConnectionFailed("Failed to find mount point".to_string()))?;

    // Copy InstantClient files
    let source_dir = PathBuf::from(mount_point).join("instantclient_21");
    if source_dir.exists() {
        copy_dir_all(source_dir, target_dir.to_path_buf()).await?;
    }

    // Unmount (ignore errors)
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet", mount_point])
        .status();

    Ok(())
}

fn copy_dir_all(source: PathBuf, target: PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        fs::create_dir_all(&target).await
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create target directory: {}", e)))?;

        let mut entries = fs::read_dir(&source).await
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read source directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            let target_path = target.join(path.file_name().unwrap());

            if path.is_dir() {
                copy_dir_all(path, target_path).await?;
            } else {
                fs::copy(&path, &target_path).await
                    .map_err(|e| SakiError::ConnectionFailed(format!("Failed to copy file: {}", e)))?;
            }
        }

        Ok(())
    })
}

fn update_library_path(instantclient_dir: &PathBuf, platform: &str) -> Result<()> {
    match platform {
        "linux-x64" => {
            if let Ok(ld_path) = env::var("LD_LIBRARY_PATH") {
                let new_path = format!("{}:{}", instantclient_dir.display(), ld_path);
                unsafe { env::set_var("LD_LIBRARY_PATH", new_path); }
            } else {
                unsafe { env::set_var("LD_LIBRARY_PATH", instantclient_dir); }
            }
        }
        "macos-arm64" | "macos-x64" => {
            if let Ok(dyld_path) = env::var("DYLD_LIBRARY_PATH") {
                let new_path = format!("{}:{}", instantclient_dir.display(), dyld_path);
                unsafe { env::set_var("DYLD_LIBRARY_PATH", new_path); }
            } else {
                unsafe { env::set_var("DYLD_LIBRARY_PATH", instantclient_dir); }
            }
        }
        "windows-x64" => {
            if let Ok(path) = env::var("PATH") {
                let new_path = format!("{};{}", instantclient_dir.display(), path);
                unsafe { env::set_var("PATH", new_path); }
            } else {
                unsafe { env::set_var("PATH", instantclient_dir); }
            }
        }
        _ => {}
    }
    Ok(())
}
