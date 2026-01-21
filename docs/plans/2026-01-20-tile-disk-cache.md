# Tile Disk Cache Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist map tiles to disk for offline viewing and faster loading on app restart.

**Architecture:** Three-tier cache (memory → disk → network). Platform-specific cache directories. LRU eviction when cache exceeds size limit. Async disk I/O to avoid blocking UI.

**Tech Stack:** Rust std::fs, std::path, platform cfg attributes, SHA256 for cache keys

---

## Task 1: Create Disk Cache Module Structure

**Files:**
- Create: `src/disk_cache.rs`
- Modify: `src/lib.rs`

**Step 1: Create the disk_cache module with platform detection**

```rust
// src/disk_cache.rs
use std::path::PathBuf;
use std::fs;
use std::io::{Read, Write};

/// Maximum cache size in bytes (50MB)
const MAX_CACHE_SIZE: u64 = 50 * 1024 * 1024;

/// Get platform-specific cache directory
pub fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        // Android: use app's cache directory via environment or fallback
        std::env::var("CACHE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from("/data/local/tmp/makepad-map")))
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
```

**Step 2: Add module to lib.rs**

In `src/lib.rs`, add:
```rust
pub mod disk_cache;
```

**Step 3: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 4: Commit**

```bash
git add src/disk_cache.rs src/lib.rs
git commit -m "feat: add disk_cache module with platform detection"
```

---

## Task 2: Implement Tile Path Generation

**Files:**
- Modify: `src/disk_cache.rs`

**Step 1: Add tile path function**

Add to `src/disk_cache.rs`:

```rust
use crate::tiles::TileCoord;

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
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/disk_cache.rs
git commit -m "feat: add tile path generation for disk cache"
```

---

## Task 3: Implement Save Tile to Disk

**Files:**
- Modify: `src/disk_cache.rs`

**Step 1: Add save_tile function**

Add to `src/disk_cache.rs`:

```rust
/// Save tile PNG data to disk
/// Returns true if saved successfully
pub fn save_tile(coord: &TileCoord, data: &[u8]) -> bool {
    let Some(path) = tile_path(coord) else {
        return false;
    };

    // Create parent directories
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
    }

    // Write the file
    match fs::File::create(&path) {
        Ok(mut file) => file.write_all(data).is_ok(),
        Err(_) => false,
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/disk_cache.rs
git commit -m "feat: add save_tile function for disk persistence"
```

---

## Task 4: Implement Load Tile from Disk

**Files:**
- Modify: `src/disk_cache.rs`

**Step 1: Add load_tile function**

Add to `src/disk_cache.rs`:

```rust
/// Load tile PNG data from disk
/// Returns None if not cached or read error
pub fn load_tile(coord: &TileCoord) -> Option<Vec<u8>> {
    let path = tile_path(coord)?;

    if !path.exists() {
        return None;
    }

    let mut file = fs::File::open(&path).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).ok()?;

    Some(data)
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/disk_cache.rs
git commit -m "feat: add load_tile function for disk cache retrieval"
```

---

## Task 5: Integrate Disk Cache with TileCache - Load on Request

**Files:**
- Modify: `src/tiles.rs`

**Step 1: Import disk_cache module**

At top of `src/tiles.rs`, add:
```rust
use crate::disk_cache;
```

**Step 2: Modify request_tile to check disk cache first**

In `TileCache::request_tile`, replace the current implementation:

```rust
/// Request a tile if not already cached or loading
pub fn request_tile(&mut self, cx: &mut Cx, coord: TileCoord) {
    // Check if already loaded or loading in memory
    if self.tiles.contains_key(&coord) {
        return;
    }

    // Check disk cache first
    if let Some(data) = disk_cache::load_tile(&coord) {
        // Try to decode from disk cache
        match ImageBuffer::from_png(&data) {
            Ok(buffer) => {
                let texture: Texture = buffer.into_new_texture(cx);
                self.tiles.insert(coord, TileState::Loaded(texture));
                return; // Successfully loaded from disk
            }
            Err(_) => {
                // Corrupted cache file, will re-download
            }
        }
    }

    // Not in disk cache, fetch from network
    self.request_counter += 1;
    let request_id = LiveId::from_num(0, self.request_counter);

    let url = coord.tile_url(&self.tile_server);
    let mut request = HttpRequest::new(url, HttpMethod::GET);
    request.set_header("User-Agent".to_string(), "MakepadMap/0.1".to_string());
    cx.http_request(request_id, request);

    self.tiles.insert(coord, TileState::Loading);
    self.pending_requests.insert(request_id, coord);
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 4: Commit**

```bash
git add src/tiles.rs
git commit -m "feat: check disk cache before network request"
```

---

## Task 6: Integrate Disk Cache with TileCache - Save on Download

**Files:**
- Modify: `src/tiles.rs`

**Step 1: Modify handle_response to save tiles to disk**

In `TileCache::handle_response`, modify the success case to save before decoding:

```rust
/// Handle HTTP response for tile loading
pub fn handle_response(&mut self, cx: &mut Cx, request_id: LiveId, response: &HttpResponse) -> bool {
    if let Some(coord) = self.pending_requests.remove(&request_id) {
        if response.status_code == 200 {
            if let Some(body) = &response.body {
                // Save to disk cache (fire and forget)
                disk_cache::save_tile(&coord, body);

                // Try to decode the PNG
                match ImageBuffer::from_png(body) {
                    Ok(buffer) => {
                        let texture: Texture = buffer.into_new_texture(cx);
                        self.tiles.insert(coord, TileState::Loaded(texture));
                        return true;
                    }
                    Err(e) => {
                        self.tiles.insert(coord, TileState::Error(format!("PNG decode error: {:?}", e)));
                    }
                }
            } else {
                self.tiles.insert(coord, TileState::Error("Empty response body".to_string()));
            }
        } else {
            self.tiles.insert(coord, TileState::Error(format!("HTTP {}", response.status_code)));
        }
    }
    false
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/tiles.rs
git commit -m "feat: save downloaded tiles to disk cache"
```

---

## Task 7: Manual Integration Test

**Files:**
- None (testing only)

**Step 1: Run the example app**

Run: `cargo run -p makepad-map-example-simple`
Expected: Map loads and displays tiles

**Step 2: Pan around the map to load tiles**

Action: Pan to a few different locations to trigger tile downloads

**Step 3: Verify disk cache was created**

Run: `ls -la ~/Library/Caches/makepad-map/tiles/` (macOS)
Or: `ls -la ~/.cache/makepad-map/tiles/` (Linux)
Expected: Directory exists with z/x/y.png structure

**Step 4: Close and reopen the app**

Action:
1. Close the app
2. Disconnect from network (airplane mode or disable wifi)
3. Reopen the app

Expected: Previously viewed tiles load from disk cache

**Step 5: Commit integration confirmation**

```bash
git commit --allow-empty -m "test: verified disk cache works on desktop"
```

---

## Task 8: Implement Cache Size Management

**Files:**
- Modify: `src/disk_cache.rs`

**Step 1: Add cache size calculation function**

Add to `src/disk_cache.rs`:

```rust
use std::time::SystemTime;

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
    let mut size = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                size += calculate_dir_size(&path);
            } else if let Ok(metadata) = entry.metadata() {
                size += metadata.len();
            }
        }
    }
    size
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/disk_cache.rs
git commit -m "feat: add cache size calculation"
```

---

## Task 9: Implement LRU Cache Eviction

**Files:**
- Modify: `src/disk_cache.rs`

**Step 1: Add eviction function**

Add to `src/disk_cache.rs`:

```rust
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
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/disk_cache.rs
git commit -m "feat: add LRU cache eviction"
```

---

## Task 10: Trigger Eviction After Saving Tiles

**Files:**
- Modify: `src/tiles.rs`

**Step 1: Add eviction call after save**

In `TileCache::handle_response`, add eviction check after saving:

```rust
// Save to disk cache (fire and forget)
disk_cache::save_tile(&coord, body);

// Periodically check cache size (every 100 tiles saved)
if self.request_counter % 100 == 0 {
    disk_cache::evict_if_needed();
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 3: Commit**

```bash
git add src/tiles.rs
git commit -m "feat: trigger cache eviction periodically"
```

---

## Task 11: Add Clear Cache Function

**Files:**
- Modify: `src/disk_cache.rs`
- Modify: `src/tiles.rs`

**Step 1: Add clear_cache function to disk_cache**

Add to `src/disk_cache.rs`:

```rust
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
```

**Step 2: Add clear method to TileCache**

In `src/tiles.rs`, modify the `clear` method:

```rust
/// Clear all cached tiles (memory and disk)
pub fn clear(&mut self) {
    self.tiles.clear();
    self.pending_requests.clear();
    disk_cache::clear_cache();
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p makepad-map`
Expected: Compiles with no errors

**Step 4: Commit**

```bash
git add src/disk_cache.rs src/tiles.rs
git commit -m "feat: add clear cache functionality"
```

---

## Task 12: Final Integration Test

**Files:**
- None (testing only)

**Step 1: Build and run on desktop**

Run: `cargo run -p makepad-map-example-simple`
Expected: App runs, tiles load

**Step 2: Pan around to cache tiles**

Action: Pan to several different locations

**Step 3: Check cache directory**

Run: `du -sh ~/Library/Caches/makepad-map/tiles/` (macOS)
Expected: Shows size of cached tiles

**Step 4: Close and reopen offline**

Action:
1. Close app
2. Disable network
3. Reopen app

Expected: Cached areas display, uncached areas show loading placeholder

**Step 5: Test on Android (if available)**

Run: `cargo makepad android run -p makepad-map-example-simple`
Expected: App runs, tiles cache and persist across restarts

**Step 6: Final commit**

```bash
git commit --allow-empty -m "test: verified disk cache works cross-platform"
```

---

## Summary

After completing all tasks, the disk cache will:
1. Persist tiles to platform-specific cache directories
2. Load from disk before network (3-tier: memory → disk → network)
3. Automatically evict old tiles when cache exceeds 50MB
4. Work on macOS, Linux, Windows, iOS, and Android
5. Provide clear cache functionality
