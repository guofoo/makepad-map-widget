# Makepad Map Widget Design

## Overview

A native map widget component for the Makepad UI framework. Uses a hybrid/texture approach with MapLibre as the rendering backend, enabling cross-platform support across all Makepad targets (iOS, Android, macOS, Windows, Linux, Web).

## Architecture

```
┌─────────────────────────────────────┐
│         Makepad Widget Layer        │  ← Markers, controls, overlays (Makepad DSL)
├─────────────────────────────────────┤
│         Texture Bridge Layer        │  ← Shares GPU texture between systems
├─────────────────────────────────────┤
│         MapLibre Renderer           │  ← Handles tiles, styling, map logic
└─────────────────────────────────────┘
```

### Why This Approach

| Approach | Stability | Cross-platform | Effort |
|----------|-----------|----------------|--------|
| Custom vector renderer | Low initially | Excellent | Massive |
| Native view embedding | High | Poor (no desktop/web SDK) | High |
| **Hybrid/texture (chosen)** | Medium-High | Good | Medium |

MapLibre was chosen because:
- Open-source (BSD license), no API keys required
- Renders via OpenGL, aligns with Makepad's GPU pipeline
- Supports all Makepad platforms
- Mature and battle-tested
- Uses OpenStreetMap data

## Platform-Specific Texture Sharing

### macOS/iOS (Metal)
- MapLibre Native supports Metal rendering
- Render to `MTLTexture` in offscreen `MTLRenderPassDescriptor`
- Zero-copy sharing via `MTLSharedEvent` for synchronization

### Android/Linux (OpenGL)
- MapLibre renders to OpenGL Framebuffer Object (FBO)
- Share texture via same GL context or `EGLImage` on Android

### Windows (DX11)
- Use ANGLE (OpenGL-to-DX11 translation) for MapLibre
- Or use `WGL_NV_DX_interop` for GL/DX texture sharing

### Web (WebGL)
- MapLibre GL JS renders to offscreen `OffscreenCanvas`
- Upload to Makepad's WebGL context via `texImage2D`

## Widget API

### DSL Usage
```rust
MapView {
    center: (-122.4194, 37.7749),
    zoom: 12.0,
    style: "https://demotiles.maplibre.org/style.json",

    on_tap: |cx, lat_lng| { /* handle tap */ },
    on_region_changed: |cx, bounds| { /* viewport changed */ },
}
```

### Properties
- `center_lng: f64`, `center_lat: f64` — map center
- `zoom: f64` — zoom level (0-22)
- `bearing: f64` — rotation in degrees
- `pitch: f64` — tilt angle (0-60)
- `style_url: &str` — MapLibre style URL or JSON

### Methods (via `MapViewRef`)
- `set_center(lng, lat, animated)`
- `set_zoom(level, animated)`
- `fit_bounds(bounds, padding)`
- `add_marker(id, lng, lat, widget)`
- `remove_marker(id)`
- `project(lng, lat) -> (x, y)`
- `unproject(x, y) -> (lng, lat)`

### Events
- `RegionChanged { center, zoom }`
- `Tapped { lat, lng }`
- `MarkerTapped { id }`
- `LongPressed { lat, lng }`

## Input Handling

| User Action | Makepad Event | MapLibre Command |
|-------------|---------------|------------------|
| Drag | `FingerMove` | `moveBy(dx, dy)` |
| Pinch | Multi-touch delta | `zoomBy(scale, center)` |
| Scroll wheel | `Scroll` | `zoomBy(delta, cursor)` |
| Double tap | `FingerDoubleTap` | `zoomIn(at: point)` |
| Single tap | `FingerUp` | `queryFeatures(at: point)` |

## Widget Structure

```rust
#[derive(Live, LiveHook, Widget)]
pub struct MapView {
    #[walk] walk: Walk,
    #[redraw] #[live] draw_bg: DrawMapTexture,
    #[animator] animator: Animator,

    #[live] center_lng: f64,
    #[live] center_lat: f64,
    #[live(12.0)] zoom: f64,
    #[live(0.0)] bearing: f64,
    #[live(0.0)] pitch: f64,
    #[live] style_url: LiveDependency,

    #[rust] map_texture: Option<Texture>,
    #[rust] maplibre: Option<MapLibreInstance>,
    #[rust] gesture_state: GestureState,
    #[rust] is_animating: bool,
}

#[derive(Clone, Debug, DefaultNone)]
pub enum MapViewAction {
    None,
    RegionChanged { center: (f64, f64), zoom: f64 },
    Tapped { lat: f64, lng: f64 },
    MarkerTapped { id: LiveId },
    LongPressed { lat: f64, lng: f64 },
}
```

## Crate Structure

```
makepad-map/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports
│   ├── map_view.rs         # Makepad widget
│   └── maplibre/
│       ├── mod.rs          # Platform dispatch
│       ├── ffi.rs          # C FFI bindings
│       ├── instance.rs     # Safe Rust wrapper
│       ├── native/         # iOS, Android, macOS, Linux, Windows
│       └── web/            # WASM/JS interop
├── examples/
│   └── simple_map.rs       # Test application
└── docs/
    └── plans/
```

## Implementation Phases

### Phase 1: Static Texture (Current)
- Widget renders a placeholder or static map image
- Validates Makepad widget integration
- No MapLibre dependency yet

### Phase 2: Gesture Handling
- Add pan/zoom gestures that update internal state
- Visual feedback (could pan a static image)

### Phase 3: Raster Tile Loading
- Load OSM raster tiles via HTTP
- Stitch tiles into texture based on viewport
- Basic map functionality without MapLibre

### Phase 4: MapLibre Integration
- FFI bindings to MapLibre Native
- Replace tile loading with full MapLibre rendering
- Vector tiles, styling, labels

### Phase 5: Full Features
- Markers as Makepad widgets
- Polylines, polygons
- Offline support
- Web platform support

## Dependencies

```toml
[dependencies]
makepad-widgets = { path = "../makepad/widgets" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["HtmlCanvasElement"] }
```

## References

- [MapLibre Native](https://github.com/mapbox/mapbox-gl-native)
- [MapLibre GL JS](https://maplibre.org/maplibre-gl-js/docs/)
- [Makepad Widgets](https://github.com/makepad/makepad/tree/main/widgets)
- [OpenMapTiles](https://openmaptiles.org/)
