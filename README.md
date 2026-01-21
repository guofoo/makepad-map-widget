# makepad-map

A cross-platform map widget for the [Makepad](https://github.com/makepad/makepad) UI framework. Displays interactive slippy maps with pan, zoom, and touch support on desktop and mobile platforms.

## Features

- Interactive map with pan and zoom
- Scroll wheel zoom (desktop)
- Pinch-to-zoom (mobile/touch)
- Double-tap to zoom in
- Configurable tile server (defaults to Carto Voyager)
- Tile caching
- Event callbacks for taps, long presses, and region changes

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
makepad-map = { path = "../makepad-map" }  # Adjust path as needed
```

Or if published to crates.io:

```toml
[dependencies]
makepad-map = "0.1"
```

## Usage

### 1. Register the widget

In your app's `live_design!` macro, link the makepad-map crate and call its `live_design` function:

```rust
use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::widgets::*;
    use makepad_map::map_view::*;  // Import the map widget

    App = {{App}} {
        ui: <Window> {
            body = <View> {
                my_map = <GeoMapView> {
                    // Optional: customize initial position
                    center_lng: -122.4194,  // San Francisco
                    center_lat: 37.7749,
                    zoom: 12.0,
                }
            }
        }
    }
}
```

### 2. Initialize in your app

Register the live design in your app startup:

```rust
impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        makepad_map::live_design(cx);  // Register map widget
    }
}
```

### 3. Handle events (optional)

Listen for map events in your `handle_event`:

```rust
fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
    let actions = self.ui.handle_widget_event(cx, event);

    for action in actions {
        // Handle tap on map
        if let GeoMapViewAction::Tapped { lng, lat } = action.cast() {
            log!("Map tapped at: {}, {}", lng, lat);
        }

        // Handle long press
        if let GeoMapViewAction::LongPressed { lng, lat } = action.cast() {
            log!("Long press at: {}, {}", lng, lat);
        }

        // Handle region change (pan/zoom)
        if let GeoMapViewAction::RegionChanged { center_lng, center_lat, zoom } = action.cast() {
            log!("Map moved to: {}, {} at zoom {}", center_lng, center_lat, zoom);
        }
    }
}
```

### 4. Control the map programmatically

```rust
// Get a reference to the map widget
let map = self.ui.geo_map_view(id!(my_map));

// Set center position
map.set_center(cx, -73.9857, 40.7484);  // New York

// Set zoom level
map.set_zoom(cx, 15.0);
```

## Configuration Options

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `center_lng` | f64 | -122.4194 | Longitude of map center |
| `center_lat` | f64 | 37.7749 | Latitude of map center |
| `zoom` | f64 | 12.0 | Zoom level (1-19) |
| `min_zoom` | f64 | 1.0 | Minimum allowed zoom |
| `max_zoom` | f64 | 19.0 | Maximum allowed zoom |
| `bearing` | f64 | 0.0 | Map rotation (not yet implemented) |
| `pitch` | f64 | 0.0 | Map tilt (not yet implemented) |

## Custom Tile Server

The widget uses [Carto Voyager](https://carto.com/basemaps/) tiles by default. To use a different tile server, modify the `TileCache` initialization or use `set_tile_server`:

```rust
// Tile URL template uses {z}, {x}, {y} placeholders
// Examples:
// OpenStreetMap: "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
// Carto Voyager: "https://a.basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}@2x.png"
// Carto Dark: "https://a.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}@2x.png"
```

Note: Some tile providers require API keys. Check the provider's terms of service.

## Running the Example

```bash
# Desktop (macOS/Linux/Windows)
cargo run -p makepad-map-example-simple

# Android
cargo makepad android run -p makepad-map-example-simple

# iOS
cargo makepad ios run -p makepad-map-example-simple
```

## Platform Support

- macOS
- Linux
- Windows
- Android
- iOS
- WebAssembly (experimental)

## License

MIT OR Apache-2.0
