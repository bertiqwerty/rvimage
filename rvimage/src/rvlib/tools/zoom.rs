use std::fmt::Debug;

use crate::{
    drawme::{Annotation, BboxAnnotation, Stroke},
    events::{Events, KeyCode},
    history::History,
    make_tool_transform,
    tools::core::Manipulate,
    types::ViewImage,
    world::World,
    GeoFig,
};
use rvimage_domain::{BbF, OutOfBoundsMode, PtF, ShapeI, TPtF};

use super::core::Mover;
const MIN_ZOOM: TPtF = 2.0;

pub fn move_zoom_box(mut mover: Mover, mut world: World, mouse_pos: Option<PtF>) -> (Mover, World) {
    let shape_orig = world.data.shape();
    let zoom_box = *world.zoom_box();
    let f_move = |mp_from, mp_to| follow_zoom_box(mp_from, mp_to, shape_orig, zoom_box);
    let opt_opt_zoom_box = mover.move_mouse_held(f_move, mouse_pos);
    if let Some(zoom_box) = opt_opt_zoom_box {
        world.set_zoom_box(zoom_box);
    }
    (mover, world)
}

fn make_zoom_on_release<P>(mp_start: P, mp_release: P) -> Option<BbF>
where
    P: Into<PtF>,
{
    let mp_start = mp_start.into();
    let mp_release = mp_release.into();
    let x_min = mp_start.x.min(mp_release.x);
    let y_min = mp_start.y.min(mp_release.y);
    let x_max = mp_start.x.max(mp_release.x);
    let y_max = mp_start.y.max(mp_release.y);

    let w = x_max - x_min;
    let h = y_max - y_min;
    if w >= MIN_ZOOM && h >= MIN_ZOOM {
        Some(BbF {
            x: x_min,
            y: y_min,
            w,
            h,
        })
    } else {
        None
    }
}

fn follow_zoom_box(
    mp_from: PtF,
    mp_to: PtF,
    shape_orig: ShapeI,
    zoom_box: Option<BbF>,
) -> Option<BbF> {
    match zoom_box {
        // we move from mp_to to mp_from since we want the image to follow the mouse
        // instead for the zoom-box to follow the mouse
        Some(zb) => match zb.follow_movement(mp_to, mp_from, shape_orig, OutOfBoundsMode::Deny) {
            Some(zb) => Some(zb),
            None => Some(zb),
        },
        _ => zoom_box,
    }
}

#[derive(Clone, Debug)]
pub struct Zoom {
    mouse_pressed_start_pos: Option<PtF>,
    mover: Mover,
    initial_view: Option<ViewImage>,
}
impl Zoom {
    fn set_mouse_start_zoom(&mut self, mp: PtF) {
        self.mouse_pressed_start_pos = Some(mp);
    }

    fn unset_mouse_start_zoom(&mut self) {
        self.mouse_pressed_start_pos = None;
        self.initial_view = None;
    }

    fn mouse_pressed(
        &mut self,
        events: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        if events.pressed(KeyCode::MouseRight) {
            self.mover.move_mouse_pressed(events.mouse_pos_on_view);
        } else if let Some(mp) = events.mouse_pos_on_orig {
            self.set_mouse_start_zoom(mp);
        }
        (world, history)
    }

    fn mouse_released_left_btn(&mut self, mut world: World, mouse_pos: Option<PtF>) -> World {
        let bx = if let (Some(mps), Some(mr)) = (self.mouse_pressed_start_pos, mouse_pos) {
            make_zoom_on_release(mps, mr).or(*world.zoom_box())
        } else {
            *world.zoom_box()
        };
        world.set_zoom_box(bx);
        world.stop_tmp_anno();
        self.unset_mouse_start_zoom();
        world
    }

    fn mouse_released(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.released(KeyCode::MouseRight) || events.held_ctrl() {
            self.unset_mouse_start_zoom();
            (world, history)
        } else if events.released(KeyCode::MouseLeft) {
            world = self.mouse_released_left_btn(world, events.mouse_pos_on_orig);
            (world, history)
        } else {
            (world, history)
        }
    }

    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.held(KeyCode::MouseRight) || events.held_ctrl() {
            (self.mover, world) = move_zoom_box(self.mover, world, events.mouse_pos_on_view);
        } else if events.held(KeyCode::MouseLeft) {
            if let (Some(mps), Some(m)) = (self.mouse_pressed_start_pos, events.mouse_pos_on_orig) {
                // animation
                let bb = BbF::from_points(mps, m);
                let white = [255, 255, 255];
                let anno = BboxAnnotation {
                    geofig: GeoFig::BB(bb),
                    fill_color: None,
                    fill_alpha: 0,
                    outline: Stroke::from_color(white),
                    outline_alpha: 255,
                    label: None,
                    is_selected: None,
                    highlight_circles: vec![],
                };
                world.request_redraw_tmp_anno(Annotation::Bbox(anno));
            }
        }
        (world, history)
    }

    fn key_pressed(
        &mut self,
        _event: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        world.set_zoom_box(None);
        (world, history)
    }
}
impl Manipulate for Zoom {
    fn new() -> Zoom {
        Zoom {
            mouse_pressed_start_pos: None,
            initial_view: None,
            mover: Mover::new(),
        }
    }
    fn events_tf(&mut self, world: World, history: History, events: &Events) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (pressed, KeyCode::MouseRight, mouse_pressed),
                (released, KeyCode::MouseLeft, mouse_released),
                (held, KeyCode::MouseLeft, mouse_held),
                (held, KeyCode::MouseRight, mouse_held),
                (pressed, KeyCode::Back, key_pressed)
            ]
        )
    }
}

#[cfg(test)]
use {image::DynamicImage, rvimage_domain::RvResult, std::collections::HashMap, std::path::Path};
#[cfg(test)]
fn mk_z(x: TPtF, y: TPtF, w: TPtF, h: TPtF) -> Option<BbF> {
    Some(BbF { x, y, w, h })
}
#[test]
fn test_make_zoom() -> RvResult<()> {
    fn test(mps: (TPtF, TPtF), mpr: (TPtF, TPtF), expected: Option<BbF>) {
        assert_eq!(make_zoom_on_release(mps, mpr), expected);
    }

    test((0.0, 0.0), (10.0, 10.0), mk_z(0.0, 0.0, 10.0, 10.0));
    test((0.0, 0.0), (100.0, 10.0), mk_z(0.0, 0.0, 100.0, 10.0));
    test((13.0, 7.0), (33.0, 17.0), mk_z(13.0, 7.0, 20.0, 10.0));
    test((5.0, 9.0), (6.0, 9.0), None);
    test((5.0, 9.0), (17.0, 19.0), mk_z(5.0, 9.0, 12.0, 10.0));

    Ok(())
}
#[test]
fn test_move_zoom() -> RvResult<()> {
    fn test(
        mpp: (usize, usize),
        mph: (usize, usize),
        zoom_box: Option<BbF>,
        expected: Option<BbF>,
    ) {
        let mpp = (mpp.0 as TPtF, mpp.1 as TPtF).into();
        let mph = (mph.0 as TPtF, mph.1 as TPtF).into();
        let shape_orig = ShapeI { w: 80, h: 80 };
        assert_eq!(follow_zoom_box(mpp, mph, shape_orig, zoom_box), expected);
    }
    test(
        (4, 4),
        (12, 8),
        mk_z(12.0, 16.0, 40.0, 40.0),
        mk_z(4.0, 12.0, 40.0, 40.0),
    );
    Ok(())
}
#[test]
fn test_on_mouse_pressed() -> RvResult<()> {
    let mouse_pos = Some((30.0, 45.0).into());
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mut z = Zoom::new();
    let prj_path = Path::new("");
    let world = World::from_real_im(im_orig, HashMap::new(), None, prj_path, None);
    let history = History::default();
    let im_orig_old = world.data.clone();
    let event = Events::default().mousepos_orig(mouse_pos);
    let (res, _) = z.mouse_pressed(&event, world, history);
    assert_eq!(res.data, im_orig_old);
    assert_eq!(z.mouse_pressed_start_pos, mouse_pos.map(|mp| mp.into()));
    Ok(())
}

#[test]
fn test_on_mouse_released() -> RvResult<()> {
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mut z = Zoom::new();
    let prj_path = Path::new("");
    let world = World::from_real_im(im_orig, HashMap::new(), None, prj_path, None);

    z.set_mouse_start_zoom((30.0, 70.0).into());

    let world = z.mouse_released_left_btn(world, Some((40.0, 80.0).into()));
    assert_eq!(
        *world.zoom_box(),
        Some(BbF {
            x: 30.0,
            y: 70.0,
            w: 10.0,
            h: 10.0
        })
    );
    assert_eq!(z.mouse_pressed_start_pos, None);
    Ok(())
}

#[test]
fn test_on_mouse_held() {}
