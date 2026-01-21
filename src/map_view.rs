use makepad_widgets::*;
use crate::tiles::{TileCache, TileCoord};

live_design! {
    link widgets;
    use link::shaders::*;
    use link::widgets::*;
    use link::theme::*;

    // Shader for rendering map tiles with UV offset/scale for parent tile fallback
    DrawMapTile = {{DrawMapTile}} {
        texture tile_texture: texture2d
        has_texture: 0.0
        uv_offset: vec2(0.0, 0.0)
        uv_scale: vec2(1.0, 1.0)

        fn pixel(self) -> vec4 {
            if self.has_texture > 0.5 {
                // Sample with UV offset and scale (for parent tile fallback)
                let uv = self.uv_offset + self.pos * self.uv_scale;
                return sample2d(self.tile_texture, uv)
            }
            // Loading placeholder - very subtle light gray
            return vec4(0.95, 0.95, 0.95, 1.0)
        }
    }

    pub GeoMapViewBase = {{GeoMapView}} {
        draw_scale_bg: {
            color: #333333
        }
        draw_scale_text: {
            color: #333333
            text_style: <THEME_FONT_REGULAR> {
                font_size: 10.0
            }
        }
        draw_attribution_bg: {
            color: #ffffffcc
        }
        draw_attribution_text: {
            color: #666666
            text_style: <THEME_FONT_REGULAR> {
                font_size: 9.0
            }
        }
    }

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
    #[live] pub uv_offset: Vec2,
    #[live] pub uv_scale: Vec2,
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

/// Scale bar step values in meters (from 10m to 1000km)
const SCALE_STEPS: &[f64] = &[
    10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
    10000.0, 20000.0, 50000.0, 100000.0, 200000.0, 500000.0, 1000000.0,
];

#[derive(Live, LiveHook, Widget)]
pub struct GeoMapView {
    #[walk] walk: Walk,
    #[redraw] #[live] pub draw_tile: DrawMapTile,

    // Scale bar drawing
    #[live] draw_scale_bg: DrawColor,
    #[live] draw_scale_text: DrawText,
    #[live(true)] pub show_scale_bar: bool,

    // Attribution overlay
    #[live] draw_attribution_bg: DrawColor,
    #[live] draw_attribution_text: DrawText,
    #[live(true)] pub show_attribution: bool,

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

    // Momentum scrolling state
    #[rust] velocity_samples: Vec<(DVec2, f64)>,  // (position, time in seconds)
    #[rust] flick_velocity: DVec2,
    #[rust] next_frame: NextFrame,
    #[rust] is_flicking: bool,

    // Momentum tunable parameters
    #[live(0.95)] pub momentum_decay: f64,
    #[live(0.5)] pub momentum_threshold: f64,

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

        // Handle momentum animation frames
        if self.next_frame.is_event(event).is_some() {
            if self.is_flicking {
                self.apply_momentum(cx, uid, &scope.path);
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
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                cx.set_key_focus(self.draw_tile.area());
                self.drag_start = Some(fe.abs);
                self.drag_start_center = Some((self.center_lng, self.center_lat));
                self.last_abs = fe.abs;

                // Stop any ongoing flick and start collecting velocity samples
                self.is_flicking = false;
                self.velocity_samples.clear();
                self.velocity_samples.push((fe.abs, fe.time));
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

                        // Add velocity sample (keep last 4)
                        self.velocity_samples.push((fe.abs, fe.time));
                        if self.velocity_samples.len() > 4 {
                            self.velocity_samples.remove(0);
                        }
                    }
                }
            }
            Hit::FingerUp(fe) if fe.is_primary_hit() => {
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

                // Calculate flick velocity from samples and start momentum if above threshold
                let velocity = self.calculate_flick_velocity();
                let speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
                if speed > self.momentum_threshold && self.initial_pinch_distance.is_none() {
                    self.flick_velocity = velocity;
                    self.is_flicking = true;
                    self.next_frame = cx.new_next_frame();
                }

                self.drag_start = None;
                self.drag_start_center = None;
                self.velocity_samples.clear();

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

                // Set up texture - try current tile, then fall back to parent tiles
                if let Some(texture) = self.tile_cache.get_tile(&coord) {
                    // Use the exact tile
                    self.draw_tile.draw_vars.set_texture(0, texture);
                    self.draw_tile.has_texture = 1.0;
                    self.draw_tile.uv_offset = Vec2 { x: 0.0, y: 0.0 };
                    self.draw_tile.uv_scale = Vec2 { x: 1.0, y: 1.0 };
                } else if let Some((parent_coord, uv_offset, uv_scale)) = self.find_parent_tile_coord(&coord) {
                    // Use scaled parent tile as fallback
                    if let Some(parent_texture) = self.tile_cache.get_tile(&parent_coord) {
                        self.draw_tile.draw_vars.set_texture(0, parent_texture);
                        self.draw_tile.has_texture = 1.0;
                        self.draw_tile.uv_offset = uv_offset;
                        self.draw_tile.uv_scale = uv_scale;
                    } else {
                        self.draw_tile.has_texture = 0.0;
                    }
                } else {
                    // No tile available, show placeholder
                    self.draw_tile.has_texture = 0.0;
                    self.draw_tile.uv_offset = Vec2 { x: 0.0, y: 0.0 };
                    self.draw_tile.uv_scale = Vec2 { x: 1.0, y: 1.0 };
                }

                // Draw the tile
                let tile_rect = Rect {
                    pos: DVec2 { x: rect.pos.x as f64 + tile_screen_x, y: rect.pos.y as f64 + tile_screen_y },
                    size: DVec2 { x: scaled_tile_size, y: scaled_tile_size },
                };
                self.draw_tile.draw_abs(cx, tile_rect);
            }
        }

        // Draw scale bar if enabled
        if self.show_scale_bar {
            let (bar_width, label) = self.calculate_scale_bar(100.0);
            let margin = 10.0;
            let bar_height = 4.0;
            let bar_y = rect.pos.y + rect.size.y - margin - bar_height;
            let bar_x = rect.pos.x + margin;

            // Draw the scale bar background (dark line)
            self.draw_scale_bg.draw_abs(cx, Rect {
                pos: dvec2(bar_x, bar_y),
                size: dvec2(bar_width, bar_height),
            });

            // Draw label above the bar
            let text_y = bar_y - 14.0; // Position text above the bar
            self.draw_scale_text.draw_abs(cx, dvec2(bar_x, text_y), &label);
        }

        // Draw attribution overlay if enabled
        if self.show_attribution {
            let attribution_text = "\u{00A9} OpenStreetMap \u{00A9} CARTO";
            let margin = 10.0;
            let padding = 4.0;

            // Estimate text dimensions based on font size and character count
            // Using approximate character width of 0.5 * font_size for small text
            let font_size = self.draw_attribution_text.text_style.font_size as f64;
            let char_count = attribution_text.chars().count() as f64;
            let text_width = char_count * font_size * 0.5;
            let text_height = font_size * 1.2; // Line height

            // Position: bottom-right with margin
            let bg_width = text_width + padding * 2.0;
            let bg_height = text_height + padding * 2.0;
            let bg_x = rect.pos.x + rect.size.x - margin - bg_width;
            let bg_y = rect.pos.y + rect.size.y - margin - bg_height;

            // Draw semi-transparent white background behind text
            self.draw_attribution_bg.draw_abs(cx, Rect {
                pos: dvec2(bg_x, bg_y),
                size: dvec2(bg_width, bg_height),
            });

            // Draw small gray text (positioned inside the background with padding)
            let text_x = bg_x + padding;
            let text_y = bg_y + padding;
            self.draw_attribution_text.draw_abs(cx, dvec2(text_x, text_y), attribution_text);
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

    /// Find a parent tile that can be used as fallback, returns (parent_coord, uv_offset, uv_scale)
    fn find_parent_tile_coord(&self, coord: &TileCoord) -> Option<(TileCoord, Vec2, Vec2)> {
        // Try parent tiles up to 4 zoom levels back
        let mut x = coord.x;
        let mut y = coord.y;
        let mut z = coord.z;

        for _ in 0..4 {
            if z == 0 {
                break;
            }

            // Move to parent coordinates
            x /= 2;
            y /= 2;
            z -= 1;

            let parent_coord = TileCoord { x, y, z };

            if self.tile_cache.get_tile(&parent_coord).is_some() {
                // Calculate UV offset and scale for the portion we need
                let zoom_diff = coord.z - z;
                let scale = 1.0 / (1 << zoom_diff) as f32;

                // Calculate which portion of the parent tile our tile occupies
                let offset_x = ((coord.x % (1 << zoom_diff)) as f32) * scale;
                let offset_y = ((coord.y % (1 << zoom_diff)) as f32) * scale;

                return Some((
                    parent_coord,
                    Vec2 { x: offset_x, y: offset_y },
                    Vec2 { x: scale, y: scale },
                ));
            }
        }
        None
    }

    /// Calculate meters per pixel at the current zoom level and latitude
    fn meters_per_pixel(&self) -> f64 {
        // Earth circumference at equator = 40075016.686 meters
        // World width in pixels = 256 * 2^zoom
        // Adjust for latitude: multiply by cos(latitude)
        let world_size_meters = 40075016.686;
        let world_size_pixels = 256.0 * 2.0_f64.powf(self.zoom);
        let meters_per_pixel_at_equator = world_size_meters / world_size_pixels;
        meters_per_pixel_at_equator * self.center_lat.to_radians().cos()
    }

    /// Calculate the scale bar width and label for a given maximum width
    fn calculate_scale_bar(&self, max_width: f64) -> (f64, String) {
        let mpp = self.meters_per_pixel();
        let max_meters = max_width * mpp;

        // Find largest step that fits within max_width
        let mut selected_meters = SCALE_STEPS[0];
        for &step in SCALE_STEPS {
            if step <= max_meters {
                selected_meters = step;
            } else {
                break;
            }
        }

        let bar_width = selected_meters / mpp;
        let label = if selected_meters >= 1000.0 {
            format!("{} km", (selected_meters / 1000.0) as i32)
        } else {
            format!("{} m", selected_meters as i32)
        };

        (bar_width, label)
    }

    /// Calculate flick velocity from position/time samples
    fn calculate_flick_velocity(&self) -> DVec2 {
        if self.velocity_samples.len() < 2 {
            return DVec2 { x: 0.0, y: 0.0 };
        }

        // Calculate velocity from the samples (pixels per second)
        let mut total_velocity = DVec2 { x: 0.0, y: 0.0 };
        let mut count = 0;

        for i in 1..self.velocity_samples.len() {
            let (pos_prev, time_prev) = self.velocity_samples[i - 1];
            let (pos_curr, time_curr) = self.velocity_samples[i];

            let dt = time_curr - time_prev;
            if dt > 0.0001 {
                // Avoid division by very small dt
                let vx = (pos_curr.x - pos_prev.x) / dt;
                let vy = (pos_curr.y - pos_prev.y) / dt;
                total_velocity.x += vx;
                total_velocity.y += vy;
                count += 1;
            }
        }

        if count > 0 {
            // Average the velocities and scale down for reasonable flick behavior
            // The scaling factor converts pixels/second to a usable per-frame velocity
            let scale = 0.016; // ~60fps frame time
            DVec2 {
                x: (total_velocity.x / count as f64) * scale,
                y: (total_velocity.y / count as f64) * scale,
            }
        } else {
            DVec2 { x: 0.0, y: 0.0 }
        }
    }

    /// Apply momentum decay and update map position
    fn apply_momentum(&mut self, cx: &mut Cx, uid: WidgetUid, path: &HeapLiveIdPath) {
        // Apply decay to velocity
        self.flick_velocity.x *= self.momentum_decay;
        self.flick_velocity.y *= self.momentum_decay;

        // Check if velocity is below threshold
        let speed = (self.flick_velocity.x * self.flick_velocity.x
            + self.flick_velocity.y * self.flick_velocity.y)
            .sqrt();

        if speed < self.momentum_threshold * 0.01 {
            // Stop flicking when velocity is very low
            self.is_flicking = false;
            self.emit_region_changed(cx, uid, path);
            return;
        }

        // Convert pixel velocity to geo coordinate delta (same logic as FingerMove)
        let world_size = TILE_SIZE * 2.0_f64.powf(self.zoom);
        let degrees_per_pixel_x = 360.0 / world_size;
        let lat_rad = self.center_lat.to_radians();
        let degrees_per_pixel_y = degrees_per_pixel_x / lat_rad.cos();

        // Apply velocity (note: negative because dragging right moves map left)
        self.center_lng -= self.flick_velocity.x * degrees_per_pixel_x;
        self.center_lat += self.flick_velocity.y * degrees_per_pixel_y;

        // Clamp latitude to valid range
        self.center_lat = self.center_lat.clamp(-85.0, 85.0);
        // Wrap longitude
        while self.center_lng > 180.0 {
            self.center_lng -= 360.0;
        }
        while self.center_lng < -180.0 {
            self.center_lng += 360.0;
        }

        // Redraw and schedule next frame
        self.draw_tile.redraw(cx);
        self.next_frame = cx.new_next_frame();
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
