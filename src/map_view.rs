use makepad_widgets::*;
use crate::tiles::{TileCache, TileCoord};

live_design! {
    link widgets;
    use link::shaders::*;
    use link::widgets::*;

    // Simple shader for rendering a single tile
    DrawMapTile = {{DrawMapTile}} {
        texture tile_texture: texture2d
        has_texture: 0.0

        fn pixel(self) -> vec4 {
            if self.has_texture > 0.5 {
                return sample2d(self.tile_texture, self.pos)
            }
            // Loading placeholder - light gray with grid
            let grid = fract(self.pos * 4.0)
            if grid.x < 0.05 || grid.y < 0.05 {
                return vec4(0.8, 0.8, 0.85, 1.0)
            }
            return vec4(0.92, 0.92, 0.92, 1.0)
        }
    }

    pub GeoMapViewBase = {{GeoMapView}} {}

    pub GeoMapView = <GeoMapViewBase> {
        width: Fill,
        height: Fill,
    }
}

#[derive(Live, LiveRegister, LiveHook)]
#[repr(C)]
pub struct DrawMapTile {
    #[deref] pub draw_super: DrawQuad,
    #[live] pub has_texture: f32,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum GeoMapViewAction {
    None,
    RegionChanged {
        center_lng: f64,
        center_lat: f64,
        zoom: f64,
    },
    Tapped {
        lng: f64,
        lat: f64,
    },
    LongPressed {
        lng: f64,
        lat: f64,
    },
}

/// Tile size in pixels (standard OSM tile size)
const TILE_SIZE: f64 = 256.0;

#[derive(Live, LiveHook, Widget)]
pub struct GeoMapView {
    #[walk] walk: Walk,
    #[redraw] #[live] pub draw_tile: DrawMapTile,

    // Map state (in geo coordinates)
    // Default to San Francisco at zoom 12
    #[live(-122.4194)] pub center_lng: f64,
    #[live(37.7749)] pub center_lat: f64,
    #[live(12.0)] pub zoom: f64,
    #[live(0.0)] pub bearing: f64,
    #[live(0.0)] pub pitch: f64,

    // Zoom constraints
    #[live(1.0)] pub min_zoom: f64,
    #[live(19.0)] pub max_zoom: f64,

    // Internal state
    #[rust] drag_start: Option<DVec2>,
    #[rust] drag_start_center: Option<(f64, f64)>,
    #[rust] last_abs: DVec2,
    #[rust] viewport_size: DVec2,

    // Pinch zoom state
    #[rust] initial_pinch_distance: Option<f64>,
    #[rust] pinch_zoom_start: Option<f64>,

    // Tile loading
    #[rust] tile_cache: TileCache,
}

impl Widget for GeoMapView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        let uid = self.widget_uid();

        // Handle HTTP responses for tile loading
        if let Event::NetworkResponses(responses) = event {
            for response in responses {
                match &response.response {
                    NetworkResponse::HttpResponse(http_response) => {
                        if self.tile_cache.handle_response(cx, response.request_id, http_response) {
                            // Tile loaded successfully, redraw
                            self.draw_tile.redraw(cx);
                        }
                    }
                    NetworkResponse::HttpRequestError(error) => {
                        self.tile_cache.handle_error(response.request_id, error);
                    }
                    _ => {}
                }
            }
        }

        // Handle touch events for pinch zoom
        if let Event::TouchUpdate(te) = event {
            // Check if we have multiple touches for pinch zoom
            if te.touches.len() >= 2 {
                // Calculate distance between first two touches
                let t0 = &te.touches[0];
                let t1 = &te.touches[1];
                let dx = t1.abs.x - t0.abs.x;
                let dy = t1.abs.y - t0.abs.y;
                let distance = (dx * dx + dy * dy).sqrt();

                if let (Some(initial_distance), Some(start_zoom)) = (self.initial_pinch_distance, self.pinch_zoom_start) {
                    // Calculate zoom change based on pinch ratio from initial
                    let scale = distance / initial_distance;
                    // Use log scale for more natural zoom feel
                    let zoom_delta = scale.ln() / std::f64::consts::LN_2;
                    let new_zoom = (start_zoom + zoom_delta).clamp(self.min_zoom, self.max_zoom);

                    if (new_zoom - self.zoom).abs() > 0.01 {
                        self.zoom = new_zoom;
                        self.draw_tile.redraw(cx);
                    }
                } else {
                    // Start of pinch - store initial state
                    self.initial_pinch_distance = Some(distance);
                    self.pinch_zoom_start = Some(self.zoom);
                }

                // Clear single-finger drag state during pinch
                self.drag_start = None;
                self.drag_start_center = None;
            }
        }

        match event.hits(cx, self.draw_tile.area()) {
            Hit::FingerDown(fe) => {
                cx.set_key_focus(self.draw_tile.area());
                self.drag_start = Some(fe.abs);
                self.drag_start_center = Some((self.center_lng, self.center_lat));
                self.last_abs = fe.abs;
            }
            Hit::FingerMove(fe) => {
                // Only handle panning if not pinching
                if self.initial_pinch_distance.is_none() {
                    if let (Some(start), Some((start_lng, start_lat))) = (self.drag_start, self.drag_start_center) {
                        let delta = fe.abs - start;

                        // Convert pixel delta to geo coordinate delta
                        // At zoom level z, the world is 256 * 2^z pixels wide
                        let world_size = TILE_SIZE * 2.0_f64.powf(self.zoom);
                        let degrees_per_pixel_x = 360.0 / world_size;

                        // Latitude scaling (Mercator projection)
                        let lat_rad = self.center_lat.to_radians();
                        let degrees_per_pixel_y = degrees_per_pixel_x / lat_rad.cos();

                        self.center_lng = start_lng - delta.x * degrees_per_pixel_x;
                        self.center_lat = start_lat + delta.y * degrees_per_pixel_y;

                        // Clamp latitude to valid range
                        self.center_lat = self.center_lat.clamp(-85.0, 85.0);
                        // Wrap longitude
                        while self.center_lng > 180.0 { self.center_lng -= 360.0; }
                        while self.center_lng < -180.0 { self.center_lng += 360.0; }

                        self.last_abs = fe.abs;
                        self.draw_tile.redraw(cx);
                    }
                }
            }
            Hit::FingerUp(fe) => {
                // Reset pinch state
                self.initial_pinch_distance = None;
                self.pinch_zoom_start = None;

                if fe.is_over && fe.tap_count == 1 && self.drag_start.is_some() {
                    // Single tap - emit tap action
                    let (lng, lat) = self.screen_to_geo(fe.abs);
                    cx.widget_action(uid, &scope.path, GeoMapViewAction::Tapped { lng, lat });
                } else if fe.is_over && fe.tap_count == 2 {
                    // Double tap - zoom in
                    self.zoom = (self.zoom + 1.0).min(self.max_zoom);
                    self.draw_tile.redraw(cx);
                    self.emit_region_changed(cx, uid, &scope.path);
                }
                self.drag_start = None;
                self.drag_start_center = None;

                self.emit_region_changed(cx, uid, &scope.path);
            }
            Hit::FingerScroll(fe) => {
                // Handle scroll wheel zoom (desktop)
                let zoom_delta = if fe.scroll.y > 0.0 { 0.5 } else { -0.5 };
                let new_zoom = (self.zoom + zoom_delta).clamp(self.min_zoom, self.max_zoom);

                if new_zoom != self.zoom {
                    self.zoom = new_zoom;
                    self.draw_tile.redraw(cx);
                    self.emit_region_changed(cx, uid, &scope.path);
                }
            }
            Hit::FingerLongPress(fe) => {
                let (lng, lat) = self.screen_to_geo(fe.abs);
                cx.widget_action(uid, &scope.path, GeoMapViewAction::LongPressed { lng, lat });
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        // Begin drawing and get the rect
        cx.begin_turtle(walk, Layout::default());
        let rect = cx.turtle().rect();
        self.viewport_size = DVec2 { x: rect.size.x as f64, y: rect.size.y as f64 };

        // Calculate tile zoom level (integer zoom for tiles)
        let tile_zoom = self.zoom.floor() as u8;
        let tile_zoom = tile_zoom.clamp(0, 19);

        // Calculate the fractional zoom for scaling tiles
        let zoom_scale = 2.0_f64.powf(self.zoom - tile_zoom as f64);

        // Calculate world coordinates of the center
        let world_size = TILE_SIZE * 2.0_f64.powf(tile_zoom as f64);
        let center_world_x = (self.center_lng + 180.0) / 360.0 * world_size;
        let lat_rad = self.center_lat.to_radians();
        let center_world_y = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * world_size;

        // Calculate which tiles are visible
        let scaled_tile_size = TILE_SIZE * zoom_scale;
        let tiles_x = (self.viewport_size.x / scaled_tile_size / 2.0).ceil() as i32 + 1;
        let tiles_y = (self.viewport_size.y / scaled_tile_size / 2.0).ceil() as i32 + 1;

        let center_tile_x = (center_world_x / TILE_SIZE).floor() as i32;
        let center_tile_y = (center_world_y / TILE_SIZE).floor() as i32;

        let max_tile = 2_i32.pow(tile_zoom as u32);

        // Calculate the offset of the center tile from the viewport center
        let center_tile_world_x = center_tile_x as f64 * TILE_SIZE;
        let center_tile_world_y = center_tile_y as f64 * TILE_SIZE;
        let offset_x = (center_world_x - center_tile_world_x) * zoom_scale;
        let offset_y = (center_world_y - center_tile_world_y) * zoom_scale;

        // Draw tiles
        for dy in -tiles_y..=tiles_y {
            for dx in -tiles_x..=tiles_x {
                let tile_x = (center_tile_x + dx).rem_euclid(max_tile);
                let tile_y = center_tile_y + dy;

                // Skip tiles outside valid y range
                if tile_y < 0 || tile_y >= max_tile {
                    continue;
                }

                let coord = TileCoord {
                    x: tile_x as u32,
                    y: tile_y as u32,
                    z: tile_zoom,
                };

                // Request tile
                self.tile_cache.request_tile(cx.cx.cx, coord);

                // Calculate tile position on screen
                let tile_screen_x = self.viewport_size.x / 2.0
                    + (dx as f64 * scaled_tile_size)
                    - offset_x;
                let tile_screen_y = self.viewport_size.y / 2.0
                    + (dy as f64 * scaled_tile_size)
                    - offset_y;

                // Set up texture if available
                if let Some(texture) = self.tile_cache.get_tile(&coord) {
                    self.draw_tile.draw_vars.set_texture(0, texture);
                    self.draw_tile.has_texture = 1.0;
                } else {
                    self.draw_tile.has_texture = 0.0;
                }

                // Draw the tile
                let tile_rect = Rect {
                    pos: DVec2 { x: rect.pos.x as f64 + tile_screen_x, y: rect.pos.y as f64 + tile_screen_y },
                    size: DVec2 { x: scaled_tile_size, y: scaled_tile_size },
                };
                self.draw_tile.draw_abs(cx, tile_rect);
            }
        }

        // End turtle and set area for hit detection
        cx.end_turtle_with_area(&mut self.draw_tile.draw_super.draw_vars.area);

        DrawStep::done()
    }
}

impl GeoMapView {
    /// Convert screen coordinates to geographic coordinates
    fn screen_to_geo(&self, screen_pos: DVec2) -> (f64, f64) {
        let tile_zoom = self.zoom.floor() as u8;
        let zoom_scale = 2.0_f64.powf(self.zoom - tile_zoom as f64);
        let world_size = TILE_SIZE * 2.0_f64.powf(tile_zoom as f64);

        // Screen offset from center
        let screen_offset_x = screen_pos.x - self.viewport_size.x / 2.0;
        let screen_offset_y = screen_pos.y - self.viewport_size.y / 2.0;

        // Convert to world coordinates
        let center_world_x = (self.center_lng + 180.0) / 360.0 * world_size;
        let lat_rad = self.center_lat.to_radians();
        let center_world_y = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * world_size;

        let world_x = center_world_x + screen_offset_x / zoom_scale;
        let world_y = center_world_y + screen_offset_y / zoom_scale;

        // Convert back to lat/lng
        let lng = world_x / world_size * 360.0 - 180.0;
        let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * world_y / world_size)).sinh().atan();
        let lat = lat_rad.to_degrees();

        (lng, lat)
    }

    fn emit_region_changed(&self, cx: &mut Cx, uid: WidgetUid, path: &HeapLiveIdPath) {
        cx.widget_action(
            uid,
            path,
            GeoMapViewAction::RegionChanged {
                center_lng: self.center_lng,
                center_lat: self.center_lat,
                zoom: self.zoom,
            },
        );
    }

    /// Set the map center programmatically
    pub fn set_center(&mut self, cx: &mut Cx, lng: f64, lat: f64) {
        self.center_lng = lng;
        self.center_lat = lat.clamp(-85.0, 85.0);
        self.draw_tile.redraw(cx);
    }

    /// Set the zoom level programmatically
    pub fn set_zoom(&mut self, cx: &mut Cx, zoom: f64) {
        self.zoom = zoom.clamp(self.min_zoom, self.max_zoom);
        self.draw_tile.redraw(cx);
    }
}

impl GeoMapViewRef {
    pub fn set_center(&self, cx: &mut Cx, lng: f64, lat: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_center(cx, lng, lat);
        }
    }

    pub fn set_zoom(&self, cx: &mut Cx, zoom: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_zoom(cx, zoom);
        }
    }
}
