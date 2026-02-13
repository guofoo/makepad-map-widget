use makepad_widgets::*;
use makepad_map::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use makepad_map::map_view::GeoMapView;

    App = {{App}} {
        ui: <Root> {
            main_window = <Window> {
                window: { title: "Makepad Map - San Francisco" },
                body = <View> {
                    flow: Down,

                    // Title bar
                    <View> {
                        width: Fill, height: 50.0
                        show_bg: true
                        draw_bg: { color: #2196F3 }
                        align: { x: 0.5, y: 0.5 }

                        <Label> {
                            width: Fit, height: Fit
                            draw_text: {
                                text_style: { font_size: 18.0 }
                                color: #ffffff
                            }
                            text: "Makepad Map"
                        }
                    }

                    <View> {
                        height: Fit,
                        flow: Right,
                        spacing: 10,
                        padding: { top: 6, bottom: 6, left: 10, right: 10 },
                        align: { y: 0.5 }

                        status_label = <Label> {
                            width: Fill,
                            height: Fit,
                            text: "Tap on map or markers"
                            draw_text: {
                                text_style: { font_size: 12.0 }
                                color: #fff
                            }
                        }
                        zoom_in_btn = <Button> {
                            text: "Zoom In (+)"
                        }
                        zoom_out_btn = <Button> {
                            text: "Zoom Out (-)"
                        }
                    }

                    // Map container - takes available space
                    <View> {
                        width: Fill,
                        height: Fill,

                        geo_map = <GeoMapView> {
                            width: Fill,
                            height: Fill,
                        }
                    }
                }
            }
        }
    }
}

app_main!(App);

#[derive(Live, LiveHook)]
pub struct App {
    #[live] ui: WidgetRef,
}

impl LiveRegister for App {
    fn live_register(cx: &mut Cx) {
        makepad_widgets::live_design(cx);
        makepad_map::live_design(cx);
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        // Add some markers to the map
        let map_ref = self.ui.geo_map_view(ids!(geo_map));

        // San Francisco landmarks with labels
        map_ref.add_marker_with_label(
            cx, live_id!(golden_gate), -122.4785, 37.8199,
            "Golden Gate", vec4(0.9, 0.2, 0.2, 1.0)  // Red
        );
        map_ref.add_marker_with_label(
            cx, live_id!(coit_tower), -122.4058, 37.8024,
            "Coit Tower", vec4(0.2, 0.5, 0.9, 1.0)  // Blue
        );
        map_ref.add_marker_with_label(
            cx, live_id!(ferry_building), -122.3935, 37.7956,
            "Ferry Building", vec4(0.2, 0.8, 0.3, 1.0)  // Green
        );
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        // Handle zoom in button
        if self.ui.button(ids!(zoom_in_btn)).clicked(actions) {
            let map_ref = self.ui.geo_map_view(ids!(geo_map));
            let new_zoom = if let Some(mut map) = map_ref.borrow_mut() {
                let z = (map.zoom + 1.0).min(map.max_zoom);
                map.set_zoom(cx, z);
                Some(z)
            } else { None };
            if let Some(z) = new_zoom {
                self.ui.label(ids!(status_label)).set_text(cx, &format!("Zoom: {:.1}", z));
            }
        }

        // Handle zoom out button
        if self.ui.button(ids!(zoom_out_btn)).clicked(actions) {
            let map_ref = self.ui.geo_map_view(ids!(geo_map));
            let new_zoom = if let Some(mut map) = map_ref.borrow_mut() {
                let z = (map.zoom - 1.0).max(map.min_zoom);
                map.set_zoom(cx, z);
                Some(z)
            } else { None };
            if let Some(z) = new_zoom {
                self.ui.label(ids!(status_label)).set_text(cx, &format!("Zoom: {:.1}", z));
            }
        }

        // Handle map actions
        let map = self.ui.geo_map_view(ids!(geo_map));

        if let Some(id) = map.marker_tapped(actions) {
            let name = if id == live_id!(golden_gate) {
                "Golden Gate Bridge"
            } else if id == live_id!(coit_tower) {
                "Coit Tower"
            } else if id == live_id!(ferry_building) {
                "Ferry Building"
            } else {
                "Unknown marker"
            };
            self.ui.label(ids!(status_label)).set_text(
                cx,
                &format!("Tapped marker: {}", name)
            );
        } else if let Some((lng, lat)) = map.tapped(actions) {
            self.ui.label(ids!(status_label)).set_text(
                cx,
                &format!("Tapped at: {:.4}, {:.4}", lat, lng)
            );
        } else if let Some((lng, lat, zoom)) = map.region_changed(actions) {
            self.ui.label(ids!(status_label)).set_text(
                cx,
                &format!("Lat: {:.4}, Lng: {:.4}, Zoom: {:.1}", lat, lng, zoom)
            );
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
