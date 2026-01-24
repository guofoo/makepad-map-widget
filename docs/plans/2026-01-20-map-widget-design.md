# Makepad Map Widget Design

## Overview

A native map widget component for the Makepad UI framework. Uses raster tile rendering with OpenStreetMap-compatible tile servers, enabling cross-platform support across all Makepad targets (iOS, Android, macOS, Windows, Linux, Web).

## Architecture

```
┌─────────────────────────────────────┐
│         Makepad Widget Layer        │  ← GeoMapView widget, overlays (scale bar, attribution)
├─────────────────────────────────────┤
│         Tile Cache Layer            │  ← Memory + disk caching, HTTP tile fetching
├─────────────────────────────────────┤
│         GPU Texture Rendering       │  ← DrawMapTile shader, texture composition
└─────────────────────────────────────┘
```

### Why Raster Tiles

| Approach | Complexity | Cross-platform | Effort |
|----------|------------|----------------|--------|
| Custom vector renderer | High | Excellent | Massive |
| Native view embedding | Low | Poor (no desktop/web SDK) | High |
| MapLibre integration | Medium | Good | Medium |
| **Raster tiles (chosen)** | Low | Excellent | Low |

Raster tiles were chosen because:
- Simple HTTP-based tile fetching
- Works on all platforms without native SDKs
- Mature ecosystem (OpenStreetMap, CARTO, etc.)
- No complex FFI bindings required
- Immediate visual results

## Current Implementation

### Core Modules

- **`src/map_view.rs`** - `GeoMapView` widget
  - `DrawMapTile` shader for GPU tile rendering
  - Pan, zoom (scroll/pinch), double-tap gestures
  - Momentum scrolling with configurable decay
  - Scale bar overlay (10m to 1000km)
  - Attribution overlay
  - **Map markers** with labels (`DrawMarker` shader, `MapMarker` struct)
  - Parent tile fallback for smooth zooming

- **`src/tiles.rs`** - Tile management
  - `TileCoord` - OSM slippy map coordinates (x, y, z)
  - `TileCache` - Memory cache + HTTP fetching
  - Web Mercator projection math

- **`src/disk_cache.rs`** - Persistent storage
  - Platform-specific cache directories
  - LRU eviction (50MB limit)
  - Three-tier caching: memory → disk → network

### Widget API

#### DSL Usage
```rust
<GeoMapView> {
    center_lng: -122.4194,  // San Francisco
    center_lat: 37.7749,
    zoom: 12.0,

    // Optional customization
    show_scale_bar: true,
    show_attribution: true,
    momentum_decay: 0.95,
}
```

#### Properties
| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `center_lng` | f64 | -122.4194 | Longitude of map center |
| `center_lat` | f64 | 37.7749 | Latitude of map center |
| `zoom` | f64 | 12.0 | Zoom level (1-19) |
| `min_zoom` | f64 | 1.0 | Minimum allowed zoom |
| `max_zoom` | f64 | 19.0 | Maximum allowed zoom |
| `marker_size` | f64 | 32.0 | Size of map markers in pixels |
| `momentum_decay` | f64 | 0.95 | Momentum decay rate (0-1) |
| `momentum_threshold` | f64 | 0.5 | Minimum velocity for momentum |
| `show_scale_bar` | bool | true | Show/hide scale bar |
| `show_attribution` | bool | true | Show/hide attribution |

#### Methods (via `GeoMapViewRef`)
- `set_center(cx, lng, lat)` - Move map center
- `set_zoom(cx, level)` - Set zoom level
- `add_marker(cx, id, lng, lat)` - Add marker at location
- `add_marker_with_color(cx, id, lng, lat, color)` - Add colored marker
- `add_marker_with_label(cx, id, lng, lat, label, color)` - Add marker with label
- `remove_marker(cx, id)` - Remove marker by ID
- `clear_markers(cx)` - Remove all markers
- `marker_count()` - Get number of markers

#### Events
- `RegionChanged { center_lng, center_lat, zoom }` - Pan/zoom completed
- `Tapped { lng, lat }` - Single tap on map
- `LongPressed { lng, lat }` - Long press on map
- `MarkerTapped { id }` - Marker was tapped

### Input Handling

| User Action | Makepad Event | Map Response |
|-------------|---------------|--------------|
| Drag | `FingerMove` | Pan map |
| Release after drag | `FingerUp` | Momentum scroll |
| Pinch | `TouchUpdate` | Zoom in/out |
| Scroll wheel | `FingerScroll` | Zoom in/out |
| Double tap | `FingerUp` (tap_count=2) | Zoom in |
| Tap on marker | `FingerUp` | Emit `MarkerTapped` |
| Tap on map | `FingerUp` | Emit `Tapped` |
| Long press | `FingerLongPress` | Emit `LongPressed` |

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| macOS | ✅ Working | Full support |
| Linux | ✅ Working | Full support |
| Windows | ✅ Working | Full support |
| iOS | ✅ Working | Full support |
| Android | ✅ Working | Full support |
| Web | 🔶 Experimental | Needs testing |

## Tile Caching

Three-tier cache with automatic eviction:

1. **Memory Cache** - HashMap of loaded textures
2. **Disk Cache** - Platform-specific directories (50MB limit)
3. **Network** - HTTP fetch from tile server

### Cache Locations

| Platform | Directory |
|----------|-----------|
| macOS | `~/Library/Caches/makepad-map/tiles/` |
| Linux | `~/.cache/makepad-map/tiles/` |
| Windows | `%LOCALAPPDATA%/makepad-map/cache/tiles/` |
| iOS | `~/Library/Caches/makepad-map/tiles/` |
| Android | `$CACHE_DIR/makepad-map/tiles/` |

## Future Enhancements

- [ ] Map rotation (bearing)
- [ ] Map tilt (pitch)
- [x] ~~Custom markers as Makepad widgets~~ → Implemented with `DrawMarker` shader and labels
- [ ] Polylines and polygons
- [ ] Vector tiles (MapLibre integration)
- [ ] Offline map packages
- [ ] Gesture animations (animated zoom/pan)

## References

- [OpenStreetMap Tile Usage Policy](https://operations.osmfoundation.org/policies/tiles/)
- [CARTO Basemaps](https://carto.com/basemaps/)
- [Slippy Map Tilenames](https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames)
- [Web Mercator Projection](https://en.wikipedia.org/wiki/Web_Mercator_projection)
