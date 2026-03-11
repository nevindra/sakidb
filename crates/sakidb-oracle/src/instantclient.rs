use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{info, error};
use dirs::{data_dir, home_dir};
use sakidb_core::error::{Result, SakiError};

// We use Oracle's permanent links ("latest") where available.
// For ARM64 macOS, Oracle only provides DMG packages.
const VERSION_LABEL: &str = "latest";

const BASE_URL_MACOS: &str = "https://download.oracle.com/otn_software/mac/instantclient";
const BASE_URL_LINUX: &str = "https://download.oracle.com/otn_software/linux/instantclient";
const BASE_URL_WINDOWS: &str = "https://download.oracle.com/otn_software/nt/instantclient";

/// Ensures Oracle Instant Client is available for the current platform.
/// Downloads and configures it automatically if not found.
pub async fn ensure_instantclient() -> Result<()> {
    // 1. Check if OCI_LIB_DIR is already set (explicit manual override)
    if let Ok(dir) = env::var("OCI_LIB_DIR") {
        if Path::new(&dir).exists() {
            info!("Using manual OCI_LIB_DIR: {}", dir);
            return Ok(());
        }
    }

    // 2. Check for Conventional Manual Install
    if is_instantclient_available() {
        info!("InstantClient detected in conventional system paths");
        return Ok(());
    }

    // 3. Setup Automatic Download (Application Support / Data Dir)
    let platform = determine_platform()?;
    let instantclient_dir = get_local_instantclient_dir(&platform)?;
    
    if !instantclient_dir.exists() {
        info!("Downloading Oracle InstantClient for platform: {}", platform);
        if let Err(e) = download_instantclient(&platform, &instantclient_dir).await {
            error!("Automatic InstantClient download failed: {}", e);
            
            let manual_guide = match platform.as_str() {
                "macos-arm64" | "macos-x64" => format!(
                    "1. Follow the official ODPI-C guide: https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html#macos\n\
                     2. OR download and manually copy contents to: {}",
                    instantclient_dir.display()
                ),
                "linux-x64" => format!(
                    "1. Follow the official ODPI-C guide: https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html#linux\n\
                     2. Extract contents to: {}\n\
                     3. OR install via your package manager (e.g., yum install oracle-instantclient-basic)",
                    instantclient_dir.display()
                ),
                "windows-x64" => format!(
                    "1. Follow the official ODPI-C guide: https://odpi-c.readthedocs.io/en/latest/user_guide/installation.html#windows\n\
                     2. Extract contents to: {}\n\
                     3. Add that directory to your system PATH.",
                    instantclient_dir.display()
                ),
                _ => "Please install Oracle Instant Client manually for your platform.".to_string(),
            };

            return Err(SakiError::ConnectionFailed(format!(
                "Failed to automatically setup Oracle Instant Client.\n\n\
                MANUAL INSTALLATION GUIDE:\n\
                {}\n\n\
                Original error: {}", 
                manual_guide, e
            )));
        }
    }

    // Set OCI_LIB_DIR environment variable to point to our internal download
    // Safety: env::set_var is unsafe in multi-threaded environments (Rust 1.81+)
    unsafe {
        env::set_var("OCI_LIB_DIR", &instantclient_dir);
    }
    info!("Set OCI_LIB_DIR to internal path: {}", instantclient_dir.display());
    
    // Update platform-specific library search paths where possible
    update_library_path(&instantclient_dir, &platform)?;
    
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
            "Unsupported platform for automatic Oracle setup: {}-{}", os, arch
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
        .join(format!("instantclient_{}", VERSION_LABEL));
    
    Ok(instantclient_dir)
}

fn is_instantclient_available() -> bool {
    // macOS
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = home_dir() {
            if home.join("lib").join("libclntsh.dylib").exists() { return true; }
        }
        if Path::new("/usr/local/lib/libclntsh.dylib").exists() { return true; }
    }

    // Linux
    #[cfg(target_os = "linux")]
    {
        if Command::new("sh").arg("-c").arg("ldconfig -p | grep libclntsh").output().is_ok_and(|o| o.status.success()) {
            return true;
        }
        for path in ["/usr/lib/libclntsh.so", "/usr/local/lib/libclntsh.so"] {
            if Path::new(path).exists() { return true; }
        }
    }

    // Windows
    #[cfg(target_os = "windows")]
    {
        if Command::new("where").arg("oci.dll").output().is_ok_and(|o| o.status.success()) {
            return true;
        }
    }

    false
}

async fn download_instantclient(platform: &str, target_dir: &PathBuf) -> Result<()> {
    fs::create_dir_all(target_dir).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create directory: {}", e)))?;

    let (download_url, filename) = match platform {
        "macos-arm64" => (
            format!("{}/instantclient-basic-macos-arm64.dmg", BASE_URL_MACOS),
            "instantclient-basic-macos-arm64.dmg"
        ),
        "macos-x64" => (
            format!("{}/instantclient-basic-macos.x64-latest.zip", BASE_URL_MACOS),
            "instantclient-basic-macos.x64.zip"
        ),
        "linux-x64" => (
            format!("{}/instantclient-basic-linux.x64-latest.zip", BASE_URL_LINUX),
            "instantclient-basic-linux.x64-latest.zip"
        ),
        "windows-x64" => (
            format!("{}/instantclient-basic-windows.x64-latest.zip", BASE_URL_WINDOWS),
            "instantclient-basic-windows.x64-latest.zip"
        ),
        _ => return Err(SakiError::ConnectionFailed(format!("Unsupported platform: {}", platform))),
    };

    info!("Downloading InstantClient from: {}", download_url);

    let response = reqwest::get(&download_url).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to download InstantClient: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SakiError::ConnectionFailed(
            format!("Failed to download InstantClient: HTTP {} from {}", response.status(), download_url)
        ));
    }

    let bytes = response.bytes().await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read download: {}", e)))?;

    let temp_file = target_dir.join(filename);
    fs::write(&temp_file, &bytes).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to write download: {}", e)))?;

    if filename.ends_with(".zip") {
        extract_zip(&temp_file, target_dir).await?;
    } else if filename.ends_with(".dmg") {
        extract_dmg(&temp_file, target_dir, platform).await?;
    }

    let _ = fs::remove_file(temp_file).await;
    info!("InstantClient setup successfully in: {}", target_dir.display());
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
        
        let temp_extract_dir = target.join("temp_extract");
        if temp_extract_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_extract_dir);
        }
        std::fs::create_dir_all(&temp_extract_dir)
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create temp extract dir: {}", e)))?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| SakiError::ConnectionFailed(format!("Failed to get file from zip: {}", e)))?;
            
            let ename = entry.name();
            let outpath = temp_extract_dir.join(ename);
            
            if !outpath.starts_with(&temp_extract_dir) {
                return Err(SakiError::ConnectionFailed(format!("Invalid zip entry path: {}", ename)));
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

        // Find the folder containing the actual libs (usually one level down)
        let mut root_dir = temp_extract_dir.clone();
        if let Ok(entries) = std::fs::read_dir(&temp_extract_dir) {
            let items: Vec<_> = entries.flatten().collect();
            if items.len() == 1 && items[0].path().is_dir() {
                root_dir = items[0].path();
            }
        }

        // Move contents to target
        if let Ok(entries) = std::fs::read_dir(&root_dir) {
            for entry in entries.flatten() {
                let from = entry.path();
                let to = target.join(from.file_name().unwrap());
                let _ = std::fs::rename(from, to);
            }
        }

        let _ = std::fs::remove_dir_all(&temp_extract_dir);
        Ok::<(), SakiError>(())
    }).await.map_err(|e| SakiError::ConnectionFailed(format!("Zip extraction task failed: {}", e)))??;
    
    Ok(())
}

async fn extract_dmg(dmg_file: &Path, target_dir: &Path, _platform: &str) -> Result<()> {
    info!("Mounting DMG: {}", dmg_file.display());
    
    // Use -plist for more robust parsing of the mount point
    let mount_output = Command::new("hdiutil")
        .args(["attach", "-plist", "-nobrowse", "-readonly", dmg_file.to_str().unwrap()])
        .output()
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to mount DMG: {}", e)))?;

    if !mount_output.status.success() {
        let stderr = String::from_utf8_lossy(&mount_output.stderr);
        error!("hdiutil attach failed: {}", stderr);
        return Err(SakiError::ConnectionFailed(format!("Failed to mount DMG: {}", stderr)));
    }

    let output_str = String::from_utf8_lossy(&mount_output.stdout);
    
    // Improved parsing using XML markers in -plist output
    let mount_point = if let Some(start) = output_str.find("<key>mount-point</key>") {
        let remaining = &output_str[start..];
        if let Some(s_start) = remaining.find("<string>") {
            let s_rem = &remaining[s_start + 8..];
            if let Some(s_end) = s_rem.find("</string>") {
                Some(s_rem[..s_end].trim().to_string())
            } else { None }
        } else { None }
    } else {
        // Fallback to searching for /Volumes/ in case -plist is weird
        output_str.lines()
            .find(|line| line.contains("/Volumes/"))
            .and_then(|line| {
                line.split("/Volumes/").last().map(|s| format!("/Volumes/{}", s.trim()))
            })
    };

    let mount_point = mount_point.ok_or_else(|| {
        error!("Failed to find mount point in hdiutil output: {}", output_str);
        SakiError::ConnectionFailed("Failed to find mount point for DMG. See logs for details.".to_string())
    })?;

    info!("DMG mounted at: {}", mount_point);

    // Find the instantclient folder in the volume
    let mut source_dir = None;
    
    // Check for folder starting with instantclient_
    if let Ok(mut entries) = std::fs::read_dir(&mount_point) {
        while let Some(Ok(entry)) = entries.next() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("instantclient_") && entry.path().is_dir() {
                source_dir = Some(entry.path());
                break;
            }
        }
    }

    // If not found, check if libraries are in the root of mount_point (common in newer ARM64 DMGs)
    if source_dir.is_none() {
        let test_lib = PathBuf::from(&mount_point).join("libclntsh.dylib");
        if test_lib.exists() {
            info!("Detected libraries directly in DMG root: {}", mount_point);
            source_dir = Some(PathBuf::from(&mount_point));
        }
    }

    let source_dir = source_dir.ok_or_else(|| {
        let _ = Command::new("hdiutil").args(["detach", "-quiet", &mount_point]).status();
        SakiError::ConnectionFailed(format!("Failed to find instantclient folder or libclntsh.dylib in DMG mount: {}", mount_point))
    })?;

    info!("Copying from DMG mount {} to {}", source_dir.display(), target_dir.display());
    copy_dir_all(source_dir, target_dir.to_path_buf()).await?;

    // Always attempt unmount
    let _ = Command::new("hdiutil").args(["detach", "-quiet", &mount_point]).status();
    
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
