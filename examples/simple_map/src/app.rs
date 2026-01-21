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
                    padding: 10,
                    spacing: 10,

                    <Label> {
                        text: "Makepad Map Widget"
                        draw_text: {
                            text_style: { font_size: 16.0 }
                        }
                    }

                    <View> {
                        height: Fit,
                        flow: Right,
                        spacing: 10,

                        zoom_in_btn = <Button> {
                            text: "Zoom In (+)"
                        }
                        zoom_out_btn = <Button> {
                            text: "Zoom Out (-)"
                        }
                    }

                    <Label> {
                        text: "Drag to pan, scroll/buttons to zoom"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: #666
                        }
                    }

                    // Map widget - defaults to San Francisco at zoom 12
                    geo_map = <GeoMapView> {
                        width: Fill,
                        height: Fill,
                    }

                    status_label = <Label> {
                        text: "San Francisco - Zoom: 12"
                        draw_text: {
                            text_style: { font_size: 10.0 }
                            color: #666
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

        // Update status when map region changes
        for action in actions {
            if let GeoMapViewAction::RegionChanged { center_lng, center_lat, zoom } = action.cast() {
                self.ui.label(ids!(status_label)).set_text(
                    cx,
                    &format!("Lat: {:.4}, Lng: {:.4}, Zoom: {:.1}", center_lat, center_lng, zoom)
                );
            }
        }
    }
}

impl AppMain for App {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
