use makepad_widgets::*;
use makepad_widgets::image_cache::ImageBuffer;
use std::collections::HashMap;

use crate::disk_cache;

/// OpenStreetMap tile coordinates
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl TileCoord {
    /// Convert latitude/longitude to tile coordinates at a given zoom level
    pub fn from_lat_lng(lat: f64, lng: f64, zoom: u8) -> Self {
        let n = 2_u32.pow(zoom as u32) as f64;
        let x = ((lng + 180.0) / 360.0 * n).floor() as u32;
        let lat_rad = lat.to_radians();
        let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor() as u32;
        Self { x, y, z: zoom }
    }

    /// Get the northwest corner of this tile in lat/lng
    pub fn to_lat_lng(&self) -> (f64, f64) {
        let n = 2_u32.pow(self.z as u32) as f64;
        let lng = self.x as f64 / n * 360.0 - 180.0;
        let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * self.y as f64 / n)).sinh().atan();
        let lat = lat_rad.to_degrees();
        (lat, lng)
    }

    /// Get OSM tile URL
    pub fn osm_url(&self) -> String {
        // Using OSM tile server - note: for production use, you should use your own tile server
        // or a provider like MapTiler, Stadia, etc.
        format!(
            "https://tile.openstreetmap.org/{}/{}/{}.png",
            self.z, self.x, self.y
        )
    }

    /// Get tile URL with custom server
    pub fn tile_url(&self, server: &str) -> String {
        server
            .replace("{z}", &self.z.to_string())
            .replace("{x}", &self.x.to_string())
            .replace("{y}", &self.y.to_string())
    }
}

/// State of a tile being loaded
#[derive(Clone)]
pub enum TileState {
    Loading,
    Loaded(Texture),
    Error(String),
}

/// Manages tile loading and caching
pub struct TileCache {
    tiles: HashMap<TileCoord, TileState>,
    pending_requests: HashMap<LiveId, TileCoord>,
    request_counter: u64,
    tile_server: String,
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            pending_requests: HashMap::new(),
            request_counter: 0,
            // Carto Voyager - clean, modern style (free, no API key required)
            tile_server: "https://a.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}@2x.png".to_string(),
        }
    }

    pub fn set_tile_server(&mut self, server: &str) {
        self.tile_server = server.to_string();
    }

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

    /// Get a tile if it's already loaded
    pub fn get_tile(&self, coord: &TileCoord) -> Option<&Texture> {
        if let Some(TileState::Loaded(texture)) = self.tiles.get(coord) {
            Some(texture)
        } else {
            None
        }
    }

    /// Handle HTTP response for tile loading
    pub fn handle_response(&mut self, cx: &mut Cx, request_id: LiveId, response: &HttpResponse) -> bool {
        if let Some(coord) = self.pending_requests.remove(&request_id) {
            if response.status_code == 200 {
                if let Some(body) = &response.body {
                    // Try to decode the PNG first (validates it's a real PNG)
                    match ImageBuffer::from_png(body) {
                        Ok(buffer) => {
                            // Save to disk cache only after successful decode
                            disk_cache::save_tile(&coord, body);

                            // Periodically check cache size (every 100 tiles saved)
                            if self.request_counter % 100 == 0 {
                                disk_cache::evict_if_needed();
                            }

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

    /// Handle HTTP error
    pub fn handle_error(&mut self, request_id: LiveId, error: &HttpError) {
        if let Some(coord) = self.pending_requests.remove(&request_id) {
            self.tiles.insert(coord, TileState::Error(format!("{:?}", error)));
        }
    }

    /// Get tiles needed to cover a viewport
    pub fn get_visible_tiles(
        center_lat: f64,
        center_lng: f64,
        zoom: u8,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Vec<TileCoord> {
        let tile_size = 256.0; // Standard OSM tile size in pixels

        // Calculate how many tiles we need
        let tiles_x = (viewport_width / tile_size).ceil() as i32 + 2;
        let tiles_y = (viewport_height / tile_size).ceil() as i32 + 2;

        let center_tile = TileCoord::from_lat_lng(center_lat, center_lng, zoom);
        let max_tile = 2_u32.pow(zoom as u32);

        let mut tiles = Vec::new();
        for dy in -(tiles_y / 2)..=(tiles_y / 2) {
            for dx in -(tiles_x / 2)..=(tiles_x / 2) {
                let x = (center_tile.x as i32 + dx).rem_euclid(max_tile as i32) as u32;
                let y = center_tile.y as i32 + dy;
                if y >= 0 && y < max_tile as i32 {
                    tiles.push(TileCoord { x, y: y as u32, z: zoom });
                }
            }
        }
        tiles
    }

    /// Clear all cached tiles (memory and disk)
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.pending_requests.clear();
        disk_cache::clear_cache();
    }
}
