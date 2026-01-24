use makepad_widgets::*;
use crate::tiles::{TileCache, TileCoord};

live_design! {
    link widgets;
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

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
            // Loading placeholder - subtle light gray
            return vec4(0.95, 0.95, 0.95, 1.0)
        }
    }

    // Shader for rendering map markers (pin/teardrop shape)
    DrawMarker = {{DrawMarker}} {
        marker_color: #ff3333

        fn pixel(self) -> vec4 {
            // Anchor at bottom point (the pin tip)
            let pos = self.pos - vec2(0.5, 0.7);

            // Teardrop: circle on top, point at bottom
            let circle_center = vec2(0.0, 0.0);
            let circle_radius = 0.3;

            // Distance to circle
            let d_circle = length(pos - circle_center) - circle_radius;

            // Triangle/cone pointing down
            let tip = vec2(0.0, 0.35);
            let d_cone = dot(pos - tip, normalize(vec2(abs(pos.x), -0.5)));

            // Combine: inside if either shape
            let d = min(d_circle, d_cone);

            if d < 0.0 {
                // Add subtle highlight for depth
                let highlight = smoothstep(0.0, -0.15, d_circle - 0.1);
                return mix(self.marker_color, vec4(1.0, 1.0, 1.0, 1.0), highlight * 0.3);
            }
            return vec4(0.0);
        }
    }

    pub GeoMapViewBase = {{GeoMapView}} {
        draw_scale_bg: {
            color: #333333
        }
        draw_scale_text: {
            color: #333333
            text_style: {
                font_size: 10.0
            }
        }
        draw_attribution_bg: {
            color: #ffffffcc
        }
        draw_attribution_text: {
            color: #666666
            text_style: {
                font_size: 9.0
            }
        }
        draw_marker_label: {
            color: #333333
            text_style: <THEME_FONT_REGULAR> {
                font_size: 11.0
            }
        }
        draw_marker_label_bg: {
            color: #ffffffee
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

#[derive(Live, LiveRegister, LiveHook)]
#[repr(C)]
pub struct DrawMarker {
    #[deref] pub draw_super: DrawQuad,
    #[live] pub marker_color: Vec4,
}

/// A marker that can be placed on the map at a geographic location
#[derive(Clone, Debug)]
pub struct MapMarker {
    pub id: LiveId,
    pub lng: f64,
    pub lat: f64,
    pub label: String,
    pub color: Vec4,
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
    MarkerTapped {
        id: LiveId,
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

    // Markers
    #[live] draw_marker: DrawMarker,
    #[live] draw_marker_label: DrawText,
    #[live] draw_marker_label_bg: DrawColor,
    #[live(32.0)] pub marker_size: f64,
    #[rust] markers: Vec<MapMarker>,

    // Map state (default: San Francisco at zoom 12)
    #[live(-122.4194)] pub center_lng: f64,
    #[live(37.7749)] pub center_lat: f64,
    #[live(12.0)] pub zoom: f64,

    // Zoom constraints
    #[live(1.0)] pub min_zoom: f64,
    #[live(19.0)] pub max_zoom: f64,

    // Internal state
    #[rust] drag_start: Option<DVec2>,
    #[rust] drag_start_center: Option<(f64, f64)>,
    #[rust] last_abs: DVec2,
    #[rust] viewport_size: DVec2,
    #[rust] viewport_pos: DVec2,  // Top-left position of viewport in absolute coords

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
        if self.next_frame.is_event(event).is_some() && self.is_flicking {
            self.apply_momentum(cx, uid, &scope.path);
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
                        let (deg_per_px_x, deg_per_px_y) = self.degrees_per_pixel();

                        self.center_lng = start_lng - delta.x * deg_per_px_x;
                        self.center_lat = start_lat + delta.y * deg_per_px_y;
                        self.normalize_coordinates();

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
                let was_pinching = self.initial_pinch_distance.is_some();
                self.initial_pinch_distance = None;
                self.pinch_zoom_start = None;

                // Check if this was a tap (minimal movement from start)
                let is_tap = if let Some(start) = self.drag_start {
                    let dist = (fe.abs - start).length();
                    dist < 10.0  // Less than 10px movement = tap
                } else {
                    false
                };

                if fe.is_over && is_tap {
                    // Check if a marker was tapped
                    if let Some(marker_id) = self.find_marker_at_screen_pos(fe.abs) {
                        cx.widget_action(uid, &scope.path, GeoMapViewAction::MarkerTapped { id: marker_id });
                    } else {
                        let (lng, lat) = self.screen_to_geo(fe.abs);
                        cx.widget_action(uid, &scope.path, GeoMapViewAction::Tapped { lng, lat });
                    }
                } else if fe.is_over && fe.tap_count == 2 {
                    self.zoom = (self.zoom + 1.0).min(self.max_zoom);
                    self.draw_tile.redraw(cx);
                }

                // Start momentum scrolling if above threshold (only for drags, not taps)
                if !is_tap && !was_pinching {
                    let velocity = self.calculate_flick_velocity();
                    if velocity.x.hypot(velocity.y) > self.momentum_threshold {
                        self.flick_velocity = velocity;
                        self.is_flicking = true;
                        self.next_frame = cx.new_next_frame();
                    }
                }

                self.drag_start = None;
                self.drag_start_center = None;
                self.velocity_samples.clear();
                if !is_tap {
                    self.emit_region_changed(cx, uid, &scope.path);
                }
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
        self.viewport_size = rect.size;
        self.viewport_pos = rect.pos;

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
                    pos: rect.pos + dvec2(tile_screen_x, tile_screen_y),
                    size: dvec2(scaled_tile_size, scaled_tile_size),
                };
                self.draw_tile.draw_abs(cx, tile_rect);
            }
        }

        // Draw markers - collect data first to avoid borrow issues
        let marker_data: Vec<_> = self.markers.iter().map(|m| {
            (self.geo_to_screen(m.lng, m.lat), m.color, m.label.clone())
        }).collect();

        for (screen_pos, color, label) in marker_data {
            // Skip if marker is off-screen (with some margin for the marker size)
            let margin = self.marker_size;
            if screen_pos.x < -margin || screen_pos.x > self.viewport_size.x + margin
                || screen_pos.y < -margin || screen_pos.y > self.viewport_size.y + margin
            {
                continue;
            }

            // Position marker so the point (bottom of pin) is at the geo location
            // The shader anchors at pos (0.5, 0.7), so we offset accordingly
            let marker_rect = Rect {
                pos: rect.pos + dvec2(
                    screen_pos.x - self.marker_size / 2.0,
                    screen_pos.y - self.marker_size * 0.7,
                ),
                size: dvec2(self.marker_size, self.marker_size),
            };

            self.draw_marker.marker_color = color;
            self.draw_marker.draw_abs(cx, marker_rect);

            // Draw label below the marker if it has one
            if !label.is_empty() {
                let text_pos = rect.pos + dvec2(screen_pos.x, screen_pos.y + 8.0);

                // Estimate text size for background
                let font_size = self.draw_marker_label.text_style.font_size as f64;
                let text_width = label.len() as f64 * font_size * 0.6;
                let text_height = font_size * 1.3;
                let padding = 3.0;

                // Draw background centered under marker
                let bg_rect = Rect {
                    pos: dvec2(text_pos.x - text_width / 2.0 - padding, text_pos.y - padding),
                    size: dvec2(text_width + padding * 2.0, text_height + padding * 2.0),
                };
                self.draw_marker_label_bg.draw_abs(cx, bg_rect);

                // Draw text centered
                self.draw_marker_label.draw_abs(cx, dvec2(text_pos.x - text_width / 2.0, text_pos.y), &label);
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
    /// Clamp latitude and wrap longitude to valid ranges
    fn normalize_coordinates(&mut self) {
        self.center_lat = self.center_lat.clamp(-85.0, 85.0);
        while self.center_lng > 180.0 { self.center_lng -= 360.0; }
        while self.center_lng < -180.0 { self.center_lng += 360.0; }
    }

    /// Get degrees per pixel at current zoom and latitude
    fn degrees_per_pixel(&self) -> (f64, f64) {
        let world_size = TILE_SIZE * 2.0_f64.powf(self.zoom);
        let deg_per_px_x = 360.0 / world_size;
        let deg_per_px_y = deg_per_px_x / self.center_lat.to_radians().cos();
        (deg_per_px_x, deg_per_px_y)
    }

    /// Convert screen coordinates to geographic coordinates
    fn screen_to_geo(&self, screen_pos: DVec2) -> (f64, f64) {
        let tile_zoom = self.zoom.floor() as u8;
        let zoom_scale = 2.0_f64.powf(self.zoom - tile_zoom as f64);
        let world_size = TILE_SIZE * 2.0_f64.powf(tile_zoom as f64);

        let center_world_x = (self.center_lng + 180.0) / 360.0 * world_size;
        let lat_rad = self.center_lat.to_radians();
        let center_world_y = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * world_size;

        let screen_offset = screen_pos - self.viewport_size / 2.0;
        let world_x = center_world_x + screen_offset.x / zoom_scale;
        let world_y = center_world_y + screen_offset.y / zoom_scale;

        let lng = world_x / world_size * 360.0 - 180.0;
        let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * world_y / world_size)).sinh().atan();
        (lng, lat_rad.to_degrees())
    }

    /// Convert geographic coordinates to screen coordinates (relative to viewport top-left)
    fn geo_to_screen(&self, lng: f64, lat: f64) -> DVec2 {
        let tile_zoom = self.zoom.floor() as u8;
        let zoom_scale = 2.0_f64.powf(self.zoom - tile_zoom as f64);
        let world_size = TILE_SIZE * 2.0_f64.powf(tile_zoom as f64);

        // Convert center to world coords
        let center_world_x = (self.center_lng + 180.0) / 360.0 * world_size;
        let center_lat_rad = self.center_lat.to_radians();
        let center_world_y = (1.0 - center_lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * world_size;

        // Convert target to world coords
        let target_world_x = (lng + 180.0) / 360.0 * world_size;
        let target_lat_rad = lat.to_radians();
        let target_world_y = (1.0 - target_lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * world_size;

        // Calculate screen offset from center
        let offset_x = (target_world_x - center_world_x) * zoom_scale;
        let offset_y = (target_world_y - center_world_y) * zoom_scale;

        // Return position relative to viewport top-left
        dvec2(
            self.viewport_size.x / 2.0 + offset_x,
            self.viewport_size.y / 2.0 + offset_y,
        )
    }

    /// Find the marker at a screen position (if any), checking in reverse order (topmost first)
    /// screen_pos should be in absolute window coordinates (as received from events)
    fn find_marker_at_screen_pos(&self, abs_pos: DVec2) -> Option<LiveId> {
        // Convert absolute position to relative viewport position
        let rel_pos = abs_pos - self.viewport_pos;

        // Hit radius covers the marker shape - use full marker size for easier tapping
        let hit_radius = self.marker_size * 0.6;

        // Check markers in reverse order (last drawn = topmost = checked first)
        for marker in self.markers.iter().rev() {
            let marker_screen = self.geo_to_screen(marker.lng, marker.lat);

            // The marker is drawn with the pin point at marker_screen, but the visible
            // head is above that point. Check against the center of the visible marker.
            let marker_center_y = marker_screen.y - self.marker_size * 0.35;

            let dx = rel_pos.x - marker_screen.x;
            let dy = rel_pos.y - marker_center_y;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance <= hit_radius {
                return Some(marker.id);
            }
        }
        None
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
            return DVec2::default();
        }

        let mut total = DVec2::default();
        let mut count = 0;

        for window in self.velocity_samples.windows(2) {
            let (pos_prev, time_prev) = window[0];
            let (pos_curr, time_curr) = window[1];
            let dt = time_curr - time_prev;
            if dt > 0.0001 {
                total += (pos_curr - pos_prev) / dt;
                count += 1;
            }
        }

        if count > 0 {
            // Scale from pixels/second to per-frame velocity (~60fps)
            total * (0.016 / count as f64)
        } else {
            DVec2::default()
        }
    }

    /// Apply momentum decay and update map position
    fn apply_momentum(&mut self, cx: &mut Cx, uid: WidgetUid, path: &HeapLiveIdPath) {
        self.flick_velocity *= self.momentum_decay;

        let speed = self.flick_velocity.x.hypot(self.flick_velocity.y);
        if speed < self.momentum_threshold * 0.01 {
            self.is_flicking = false;
            self.emit_region_changed(cx, uid, path);
            return;
        }

        let (deg_per_px_x, deg_per_px_y) = self.degrees_per_pixel();
        self.center_lng -= self.flick_velocity.x * deg_per_px_x;
        self.center_lat += self.flick_velocity.y * deg_per_px_y;
        self.normalize_coordinates();

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

    /// Add a marker at the specified geographic coordinates
    /// Returns a mutable reference to the marker for further customization
    pub fn add_marker(&mut self, cx: &mut Cx, id: LiveId, lng: f64, lat: f64) -> &mut MapMarker {
        // Default red color for markers
        let marker = MapMarker {
            id,
            lng,
            lat,
            label: String::new(),
            color: vec4(0.9, 0.2, 0.2, 1.0), // Default red
        };
        self.markers.push(marker);
        self.draw_tile.redraw(cx);
        self.markers.last_mut().unwrap()
    }

    /// Remove a marker by ID
    pub fn remove_marker(&mut self, cx: &mut Cx, id: LiveId) {
        self.markers.retain(|m| m.id != id);
        self.draw_tile.redraw(cx);
    }

    /// Get a reference to a marker by ID
    pub fn get_marker(&self, id: LiveId) -> Option<&MapMarker> {
        self.markers.iter().find(|m| m.id == id)
    }

    /// Get a mutable reference to a marker by ID
    pub fn get_marker_mut(&mut self, id: LiveId) -> Option<&mut MapMarker> {
        self.markers.iter_mut().find(|m| m.id == id)
    }

    /// Remove all markers
    pub fn clear_markers(&mut self, cx: &mut Cx) {
        self.markers.clear();
        self.draw_tile.redraw(cx);
    }

    /// Get the number of markers
    pub fn marker_count(&self) -> usize {
        self.markers.len()
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

    /// Add a marker at the specified geographic coordinates
    pub fn add_marker(&self, cx: &mut Cx, id: LiveId, lng: f64, lat: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.add_marker(cx, id, lng, lat);
        }
    }

    /// Add a marker with a custom color
    pub fn add_marker_with_color(&self, cx: &mut Cx, id: LiveId, lng: f64, lat: f64, color: Vec4) {
        if let Some(mut inner) = self.borrow_mut() {
            let marker = inner.add_marker(cx, id, lng, lat);
            marker.color = color;
        }
    }

    /// Add a marker with label and color
    pub fn add_marker_with_label(&self, cx: &mut Cx, id: LiveId, lng: f64, lat: f64, label: &str, color: Vec4) {
        if let Some(mut inner) = self.borrow_mut() {
            let marker = inner.add_marker(cx, id, lng, lat);
            marker.label = label.to_string();
            marker.color = color;
        }
    }

    /// Remove a marker by ID
    pub fn remove_marker(&self, cx: &mut Cx, id: LiveId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.remove_marker(cx, id);
        }
    }

    /// Remove all markers
    pub fn clear_markers(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear_markers(cx);
        }
    }

    /// Get the number of markers
    pub fn marker_count(&self) -> usize {
        if let Some(inner) = self.borrow() {
            inner.marker_count()
        } else {
            0
        }
    }

    /// Check if the map was tapped (returns coordinates if tapped)
    pub fn tapped(&self, actions: &Actions) -> Option<(f64, f64)> {
        if let GeoMapViewAction::Tapped { lng, lat } = actions.find_widget_action(self.widget_uid()).cast() {
            Some((lng, lat))
        } else {
            None
        }
    }

    /// Check if a marker was tapped (returns marker ID if tapped)
    pub fn marker_tapped(&self, actions: &Actions) -> Option<LiveId> {
        if let GeoMapViewAction::MarkerTapped { id } = actions.find_widget_action(self.widget_uid()).cast() {
            Some(id)
        } else {
            None
        }
    }

    /// Check if the map region changed (returns new center and zoom)
    pub fn region_changed(&self, actions: &Actions) -> Option<(f64, f64, f64)> {
        if let GeoMapViewAction::RegionChanged { center_lng, center_lat, zoom } = actions.find_widget_action(self.widget_uid()).cast() {
            Some((center_lng, center_lat, zoom))
        } else {
            None
        }
    }
}
