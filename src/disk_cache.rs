use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::tiles::TileCoord;

/// Maximum cache size in bytes (50MB)
const MAX_CACHE_SIZE: u64 = 50 * 1024 * 1024;

/// Get platform-specific cache directory
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // Android: use app's cache directory via environment variable
        // The app must set CACHE_DIR to the app's cache directory
        std::env::var("CACHE_DIR")
            .ok()
            .map(|p| PathBuf::from(p).join("makepad-map"))
    }

    #[cfg(target_os = "ios")]
    {
        // iOS: Library/Caches
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Caches/makepad-map"))
    }

    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library/Caches/makepad-map"))
    }

    #[cfg(target_os = "linux")]
    {
        std::env::var("XDG_CACHE_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".cache")))
            .map(|p| p.join("makepad-map"))
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .ok()
            .map(|p| PathBuf::from(p).join("makepad-map").join("cache"))
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "ios",
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    )))]
    {
        None
    }
}

/// Generate cache file path for a tile
/// Format: {cache_dir}/tiles/{z}/{x}/{y}.png
pub fn tile_path(coord: &TileCoord) -> Option<PathBuf> {
    cache_dir().map(|base| {
        base.join("tiles")
            .join(coord.z.to_string())
            .join(coord.x.to_string())
            .join(format!("{}.png", coord.y))
    })
}

/// Save tile PNG data to disk
pub fn save_tile(coord: &TileCoord, data: &[u8]) -> bool {
    let Some(path) = tile_path(coord) else { return false };
    path.parent()
        .and_then(|p| fs::create_dir_all(p).ok())
        .and_then(|_| fs::write(&path, data).ok())
        .is_some()
}

/// Load tile PNG data from disk
pub fn load_tile(coord: &TileCoord) -> Option<Vec<u8>> {
    fs::read(tile_path(coord)?).ok()
}

/// Get total size of cache directory in bytes
pub fn cache_size() -> u64 {
    let Some(base) = cache_dir() else {
        return 0;
    };

    let tiles_dir = base.join("tiles");
    if !tiles_dir.exists() {
        return 0;
    }

    calculate_dir_size(&tiles_dir)
}

fn calculate_dir_size(path: &PathBuf) -> u64 {
    fs::read_dir(path).into_iter().flatten().flatten().fold(0, |acc, entry| {
        let p = entry.path();
        acc + if p.is_dir() { calculate_dir_size(&p) } else { entry.metadata().map(|m| m.len()).unwrap_or(0) }
    })
}

/// Evict oldest files until cache is under MAX_CACHE_SIZE
/// Call this periodically (e.g., on app startup or after saving tiles)
pub fn evict_if_needed() {
    let current_size = cache_size();
    if current_size <= MAX_CACHE_SIZE {
        return;
    }

    let Some(base) = cache_dir() else {
        return;
    };

    let tiles_dir = base.join("tiles");
    if !tiles_dir.exists() {
        return;
    }

    // Collect all tile files with their modification times
    let mut files: Vec<(PathBuf, SystemTime)> = Vec::new();
    collect_files_with_times(&tiles_dir, &mut files);

    // Sort by modification time (oldest first)
    files.sort_by(|a, b| a.1.cmp(&b.1));

    // Delete oldest files until under limit
    let mut size = current_size;
    for (path, _) in files {
        if size <= MAX_CACHE_SIZE {
            break;
        }
        if let Ok(metadata) = fs::metadata(&path) {
            let file_size = metadata.len();
            if fs::remove_file(&path).is_ok() {
                size = size.saturating_sub(file_size);
            }
        }
    }

    // Clean up empty directories
    cleanup_empty_dirs(&tiles_dir);
}

fn collect_files_with_times(dir: &PathBuf, files: &mut Vec<(PathBuf, SystemTime)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_with_times(&path, files);
            } else if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    files.push((path, modified));
                }
            }
        }
    }
}

fn cleanup_empty_dirs(dir: &PathBuf) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                cleanup_empty_dirs(&path);
                // Try to remove if empty (will fail silently if not empty)
                let _ = fs::remove_dir(&path);
            }
        }
    }
}

/// Clear all cached tiles
pub fn clear_cache() {
    let Some(base) = cache_dir() else {
        return;
    };

    let tiles_dir = base.join("tiles");
    if tiles_dir.exists() {
        let _ = fs::remove_dir_all(&tiles_dir);
    }
}
