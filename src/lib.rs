pub use makepad_widgets;
pub use makepad_widgets::*;

pub mod map_view;
pub mod tiles;

pub use map_view::*;
pub use tiles::*;

pub fn live_design(cx: &mut Cx) {
    crate::map_view::live_design(cx);
}
