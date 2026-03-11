use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::info;
use dirs::{data_dir, home_dir};
use sakidb_core::error::{Result, SakiError};
use serde::Serialize;

// We use Oracle's permanent links ("latest") where available.
// For ARM64 macOS, Oracle only provides DMG packages.
const VERSION_LABEL: &str = "latest";

const BASE_URL_MACOS: &str = "https://download.oracle.com/otn_software/mac/instantclient";
const BASE_URL_LINUX: &str = "https://download.oracle.com/otn_software/linux/instantclient";
const BASE_URL_WINDOWS: &str = "https://download.oracle.com/otn_software/nt/instantclient";

#[derive(Debug, Clone, Serialize)]
pub struct OracleDriverStatus {
    pub found: bool,
    pub path: Option<String>,
    pub method: Option<String>, // "env", "system", "data_dir", "manual"
}

/// Checks the status of the Oracle driver in all conventional and requested locations.
pub fn get_driver_status() -> OracleDriverStatus {
    info!("Checking Oracle driver status...");
    // 1. Check if OCI_LIB_DIR is already set
    if let Ok(dir) = env::var("OCI_LIB_DIR") {
        let path = Path::new(&dir);
        if path.exists() && is_lib_present(path) {
            info!("Found Oracle driver via OCI_LIB_DIR: {}", dir);
            return OracleDriverStatus {
                found: true,
                path: Some(dir),
                method: Some("env_oci_lib_dir".to_string()),
            };
        }
        info!("OCI_LIB_DIR is set but path doesn't exist or lib missing: {}", dir);
    }

    // 2. Check ORACLE_HOME
    if let Ok(dir) = env::var("ORACLE_HOME") {
        let path = Path::new(&dir);
        if path.exists() {
            // Check for lib subfolder
            let lib_path = path.join("lib");
            if lib_path.exists() && is_lib_present(&lib_path) {
                info!("Found Oracle driver via ORACLE_HOME/lib: {}", lib_path.display());
                return OracleDriverStatus {
                    found: true,
                    path: Some(lib_path.to_string_lossy().to_string()),
                    method: Some("env_oracle_home".to_string()),
                };
            }
            if is_lib_present(path) {
                info!("Found Oracle driver via ORACLE_HOME: {}", dir);
                return OracleDriverStatus {
                    found: true,
                    path: Some(dir),
                    method: Some("env_oracle_home".to_string()),
                };
            }
            info!("ORACLE_HOME is set but libraries missing in {}", dir);
        } else {
            info!("ORACLE_HOME is set but path doesn't exist: {}", dir);
        }
    }

    // 3. Check conventional system paths
    let system_paths = get_conventional_paths();
    for path_str in system_paths {
        let path = Path::new(&path_str);
        if path.exists() {
            // If it's a file, we want the directory
            let dir = if path.is_file() {
                path.parent().unwrap_or(path).to_path_buf()
            } else {
                path.to_path_buf()
            };
            
            if is_lib_present(&dir) {
                info!("Found Oracle driver in system path: {}", dir.display());
                return OracleDriverStatus {
                    found: true,
                    path: Some(dir.to_string_lossy().to_string()),
                    method: Some("system".to_string()),
                };
            }
        }
    }

    // 4. Check data_dir (Internal download)
    if let Ok(platform) = determine_platform() {
        if let Ok(instantclient_dir) = get_local_instantclient_dir(&platform) {
            if instantclient_dir.exists() && is_lib_present(&instantclient_dir) {
                info!("Found Oracle driver in internal data dir: {}", instantclient_dir.display());
                return OracleDriverStatus {
                    found: true,
                    path: Some(instantclient_dir.to_string_lossy().to_string()),
                    method: Some("data_dir".to_string()),
                };
            }
            info!("Internal data dir check: missing or libraries not present in {}", instantclient_dir.display());
        }
    }

    info!("Oracle driver not found in any standard location.");
    OracleDriverStatus {
        found: false,
        path: None,
        method: None,
    }
}

fn is_lib_present(dir: &Path) -> bool {
    let lib_name = if cfg!(windows) { 
        "oci.dll" 
    } else if cfg!(target_os = "linux") { 
        "libclntsh.so" 
    } else { 
        "libclntsh.dylib" 
    };
    dir.join(lib_name).exists()
}

fn get_conventional_paths() -> Vec<String> {
    let mut paths = Vec::new();

    #[cfg(target_os = "macos")]
    {
        // Homebrew ARM
        // paths.push("/opt/homebrew/lib".to_string());
        // Conventional Oracle path
        paths.push("/opt/oracle/instantclient".to_string());
        // Home lib
        if let Some(home) = home_dir() {
            paths.push(home.join("lib").to_string_lossy().to_string());
        }
        // Global convention
        paths.push("/usr/local/lib".to_string());
        // Caskroom
        paths.push("/usr/local/Caskroom/oracle-instantclient".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        paths.push("/usr/lib".to_string());
        paths.push("/usr/local/lib".to_string());
        paths.push("/usr/lib/oracle/current/client64/lib".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        paths.push("C:\\oracle\\instantclient".to_string());
    }

    paths
}

/// Ensures Oracle Instant Client is available and correctly configured for dynamic loading.
pub async fn ensure_instantclient() -> Result<()> {
    let status = get_driver_status();
    if status.found {
        let path = status.path.unwrap();
        let path_buf = PathBuf::from(&path);

        // On macOS, we must ensure dyld can see the library
        #[cfg(target_os = "macos")]
        ensure_dyld_visibility(&path_buf).ok(); // Non-fatal if we can't symlink

        // Safety: env::set_var is unsafe in multi-threaded environments
        unsafe {
            env::set_var("OCI_LIB_DIR", &path);
        }
        info!("Oracle Client configured via {}: {}", status.method.unwrap_or_default(), path);
        
        let platform = determine_platform()?;
        update_library_path(&path_buf, &platform)?;
        return Ok(());
    }

    Err(SakiError::ConnectionFailed("Oracle Instant Client not found. Please download it via the connection dialog.".to_string()))
}

/// On macOS, SIP and dyld restrictions often ignore OCI_LIB_DIR at runtime.
/// Creating a symlink in ~/lib is the most reliable way to make libraries visible to dlopen.
#[cfg(target_os = "macos")]
fn ensure_dyld_visibility(source_dir: &Path) -> std::io::Result<()> {
    let lib_name = "libclntsh.dylib";
    let source_lib = source_dir.join(lib_name);
    
    if !source_lib.exists() {
        return Ok(());
    }

    let home = home_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No home dir"))?;
    let user_lib_dir = home.join("lib");
    
    if !user_lib_dir.exists() {
        std::fs::create_dir_all(&user_lib_dir)?;
    }

    let target_link = user_lib_dir.join(lib_name);
    
    // If it exists but points elsewhere or is broken, remove it
    if target_link.exists() || target_link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(&target_link);
    }

    info!("Creating dyld symlink: {} -> {}", target_link.display(), source_lib.display());
    std::os::unix::fs::symlink(source_lib, target_link)?;
    
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

pub async fn download_instantclient_with_progress<F>(on_progress: F) -> Result<()> 
where F: Fn(f64, &str) + Send + Sync + 'static
{
    let platform = determine_platform()?;
    let target_dir = get_local_instantclient_dir(&platform)?;

    fs::create_dir_all(&target_dir).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to create directory: {}", e)))?;

    let (download_url, filename) = match platform.as_str() {
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

    on_progress(0.0, "Downloading...");
    info!("Downloading InstantClient from: {}", download_url);

    let client = reqwest::Client::new();
    let response = client.get(&download_url).send().await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to download InstantClient: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SakiError::ConnectionFailed(
            format!("Failed to download InstantClient: HTTP {} from {}", response.status(), download_url)
        ));
    }

    let total_size = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::with_capacity(total_size as usize);

    use futures_util::StreamExt;
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| SakiError::ConnectionFailed(format!("Download error: {}", e)))?;
        bytes.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;
        
        if total_size > 0 {
            let p = (downloaded as f64 / total_size as f64) * 100.0;
            on_progress(p * 0.8, &format!("Downloading... {:.1}%", p)); 
        }
    }

    let temp_file = target_dir.join(filename);
    fs::write(&temp_file, &bytes).await
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to write download: {}", e)))?;

    on_progress(85.0, "Extracting...");
    if filename.ends_with(".zip") {
        extract_zip(&temp_file, &target_dir).await?;
    } else if filename.ends_with(".dmg") {
        extract_dmg(&temp_file, &target_dir, &platform).await?;
    }

    let _ = fs::remove_file(temp_file).await;
    
    // Ensure visibility after download
    #[cfg(target_os = "macos")]
    ensure_dyld_visibility(&target_dir).ok();

    on_progress(100.0, "Setup complete");
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
                std::fs::create_dir_all(&outpath).ok();
            } else {
                if let Some(p) = outpath.parent() { std::fs::create_dir_all(p).ok(); }
                let outfile = std::fs::File::create(&outpath).ok();
                if let Some(mut f) = outfile {
                    let _ = std::io::copy(&mut entry, &mut f);
                }
            }
        }

        flatten_directory(&temp_extract_dir, &target);
        let _ = std::fs::remove_dir_all(&temp_extract_dir);
        Ok::<(), SakiError>(())
    }).await.map_err(|e| SakiError::ConnectionFailed(format!("Zip extraction task failed: {}", e)))??;
    Ok(())
}

fn flatten_directory(src: &Path, dst: &Path) {
    let mut root_dir = src.to_path_buf();
    if let Ok(entries) = std::fs::read_dir(src) {
        let items: Vec<_> = entries.flatten().collect();
        if items.len() == 1 && items[0].path().is_dir() {
            root_dir = items[0].path();
        }
    }

    if let Ok(entries) = std::fs::read_dir(&root_dir) {
        for entry in entries.flatten() {
            let from = entry.path();
            let to = dst.join(from.file_name().unwrap());
            let _ = std::fs::rename(from, to);
        }
    }
}

async fn extract_dmg(dmg_file: &Path, target_dir: &Path, _platform: &str) -> Result<()> {
    info!("Mounting DMG: {}", dmg_file.display());
    let mount_output = Command::new("hdiutil")
        .args(["attach", "-plist", "-nobrowse", "-readonly", dmg_file.to_str().unwrap()])
        .output()
        .map_err(|e| SakiError::ConnectionFailed(format!("Failed to mount DMG: {}", e)))?;

    if !mount_output.status.success() {
        let stderr = String::from_utf8_lossy(&mount_output.stderr);
        return Err(SakiError::ConnectionFailed(format!("Failed to mount DMG: {}", stderr)));
    }

    let output_str = String::from_utf8_lossy(&mount_output.stdout);
    let mount_point = if let Some(start) = output_str.find("<key>mount-point</key>") {
        let remaining = &output_str[start..];
        remaining.find("<string>").and_then(|s_start| {
            let s_rem = &remaining[s_start + 8..];
            s_rem.find("</string>").map(|s_end| s_rem[..s_end].trim().to_string())
        })
    } else {
        output_str.lines().find(|line| line.contains("/Volumes/")).and_then(|line| {
            line.split("/Volumes/").last().map(|s| format!("/Volumes/{}", s.trim()))
        })
    };

    let mount_point = mount_point.ok_or_else(|| SakiError::ConnectionFailed("Failed to find mount point for DMG".to_string()))?;
    info!("DMG mounted at: {}", mount_point);

    let mut source_dir = None;
    if let Ok(mut entries) = std::fs::read_dir(&mount_point) {
        while let Some(Ok(entry)) = entries.next() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("instantclient_") && entry.path().is_dir() {
                source_dir = Some(entry.path());
                break;
            }
        }
    }

    if source_dir.is_none() {
        let test_lib = PathBuf::from(&mount_point).join("libclntsh.dylib");
        if test_lib.exists() {
            source_dir = Some(PathBuf::from(&mount_point));
        }
    }

    let source_dir = source_dir.ok_or_else(|| {
        let _ = Command::new("hdiutil").args(["detach", "-quiet", &mount_point]).status();
        SakiError::ConnectionFailed(format!("Failed to find instantclient folder or libclntsh.dylib in DMG mount: {}", mount_point))
    })?;

    info!("Copying from DMG mount {} to {}", source_dir.display(), target_dir.display());
    copy_dir_all(source_dir, target_dir.to_path_buf()).await?;
    let _ = Command::new("hdiutil").args(["detach", "-quiet", &mount_point]).status();
    Ok(())
}

fn copy_dir_all(source: PathBuf, target: PathBuf) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        let _ = fs::create_dir_all(&target).await;
        let mut entries = fs::read_dir(&source).await
            .map_err(|e| SakiError::ConnectionFailed(format!("Failed to read source directory: {}", e)))?;
        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            let path = entry.path();
            let target_path = target.join(path.file_name().unwrap());
            if path.is_dir() {
                copy_dir_all(path, target_path).await?;
            } else {
                let _ = fs::copy(&path, &target_path).await;
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
