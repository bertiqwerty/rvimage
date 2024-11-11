use std::fmt::Debug;

use rvimage_domain::{BbF, BbI, ShapeF, ShapeI};

use crate::{
    events::{Events, KeyCode, ZoomAmount},
    history::History,
    make_tool_transform,
    tools::core::Manipulate,
    world::World,
};

use super::{core::Mover, zoom::move_zoom_box};

fn event_move_zoom_box(events: &Events) -> bool {
    events.held_ctrl() && (events.pressed(KeyCode::MouseLeft) || events.held(KeyCode::MouseLeft))
}
fn zoom_box_mouse_wheel(
    zoom_box: Option<BbF>,
    shape_orig: ShapeI,
    ui_image_rect: Option<ShapeF>,
    amount: ZoomAmount,
) -> BbF {
    let current_zb = if let Some(zb) = zoom_box {
        zb
    } else {
        BbI::from_arr(&[0, 0, shape_orig.w, shape_orig.h]).into()
    };
    let clip_val = 1.0;
    let factor = match amount {
        ZoomAmount::Delta(y_delta) => {
            let y_delta_clipped = if y_delta > 0.0 {
                y_delta.min(clip_val)
            } else {
                y_delta.max(-clip_val)
            };
            1.0 - y_delta_clipped * 0.1
        }
        ZoomAmount::Factor(factor) => factor,
    };
    let vis_im_shape = current_zb.shape();
    let ar = |s: ShapeF| s.h / s.w;
    let (x_factor, y_factor) = match ui_image_rect {
        Some(uir) => {
            let diff2off = |ar_diff: f64| (ar_diff / 5.0).abs().min(0.1);
            let ar_diff = ar(uir) - ar(vis_im_shape);
            let (xoff, yoff) = if ar_diff > 0.0 {
                let off = diff2off(ar_diff);
                (off, -off)
            } else if ar_diff < 0.0 {
                let off = diff2off(ar_diff);
                (-off, off)
            } else {
                (0.0, 0.0)
            };
            if factor < 1.0 {
                ((factor - xoff).max(0.02), (factor - yoff).max(0.02))
            } else {
                (factor, factor)
            }
        }
        None => (factor, factor),
    };
    current_zb.center_scale(x_factor, y_factor, shape_orig)
}

#[derive(Clone, Debug)]
pub struct AlwaysActiveZoom {
    mover: Mover,
}
impl AlwaysActiveZoom {
    fn mouse_pressed(
        &mut self,
        events: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        if event_move_zoom_box(events) {
            self.mover.move_mouse_pressed(events.mouse_pos_on_view);
        }
        (world, history)
    }

    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event_move_zoom_box(events) {
            (self.mover, world) = move_zoom_box(self.mover, world, events.mouse_pos_on_view);
            (world, history)
        } else {
            (world, history)
        }
    }

    fn key_released(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.held_ctrl() {
            let zb = if events.released(KeyCode::Key0) {
                None
            } else if events.released(KeyCode::PlusEquals) {
                Some(zoom_box_mouse_wheel(
                    *world.zoom_box(),
                    world.shape_orig(),
                    world.ui_image_rect(),
                    ZoomAmount::Delta(1.0),
                ))
            } else if events.released(KeyCode::Minus) {
                Some(zoom_box_mouse_wheel(
                    *world.zoom_box(),
                    world.shape_orig(),
                    world.ui_image_rect(),
                    ZoomAmount::Delta(-1.0),
                ))
            } else {
                *world.zoom_box()
            };
            world.set_zoom_box(zb);
        }
        (world, history)
    }
}
impl Manipulate for AlwaysActiveZoom {
    fn new() -> AlwaysActiveZoom {
        AlwaysActiveZoom {
            mover: Mover::new(),
        }
    }
    fn has_been_used(&self, events: &Events) -> Option<bool> {
        let zoomed = events.zoom().is_some()
            || events.held_ctrl()
                && (events.released(KeyCode::Key0)
                    || events.released(KeyCode::PlusEquals)
                    || events.released(KeyCode::Minus));
        Some(zoomed || event_move_zoom_box(events))
    }
    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        events: &Events,
    ) -> (World, History) {
        let zoom_factor = events.zoom();
        if let Some(z) = zoom_factor {
            let zb = zoom_box_mouse_wheel(
                *world.zoom_box(),
                world.shape_orig(),
                world.ui_image_rect(),
                z,
            );
            world.set_zoom_box(Some(zb));
        }
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (held, KeyCode::MouseLeft, mouse_held),
                (released, KeyCode::Key0, key_released),
                (released, KeyCode::PlusEquals, key_released), // Plus is equals
                (released, KeyCode::Minus, key_released)
            ]
        )
    }
}

#[test]
fn test_zb() {
    fn test(zb: Option<BbF>, y_delta: f64, reference_coords: &[u32; 4]) {
        println!("y_delta {}", y_delta);
        let shape = ShapeI::new(200, 100);
        let zb_new = zoom_box_mouse_wheel(zb, shape, None, ZoomAmount::Delta(y_delta));
        assert_eq!(zb_new, BbI::from_arr(reference_coords).into());
    }
    test(None, 1.0, &[10, 5, 180, 90]);
    test(None, -1.0, &[0, 0, 200, 100]);
}
