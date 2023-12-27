use std::{cmp::Ordering, iter::empty, mem};

use crate::{
    cfg::CocoFile,
    domain::{shape_unscaled, OutOfBoundsMode, Point, PtF, PtI, BB},
    events::{Events, KeyCode},
    file_util::MetaData,
    history::Record,
    tools::{core::Mover, BBOX_NAME},
    tools_data::{self, annotations::SplitMode, bbox_data::ClipboardData, BboxSpecificData},
    util::true_indices,
    GeoFig, Polygon,
    {history::History, world::World},
};

use super::core::{
    are_boxes_visible, current_cat_idx, get_annos, get_annos_if_some, get_annos_mut, get_options,
    get_options_mut, get_specific_mut, paste, ACTOR_NAME,
};

const CORNER_TOL_DENOMINATOR: u32 = 5000;

fn find_closest_boundary_idx(pos: PtF, geos: &[GeoFig]) -> Option<usize> {
    geos.iter()
        .enumerate()
        .filter(|(_, geo)| geo.contains(pos))
        .map(|(i, geo)| (i, geo.distance_to_boundary(pos)))
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
        .map(|(i, _)| i)
}

/// returns index of the bounding box and the index of the closest close corner
fn find_close_vertex(orig_pos: PtF, geos: &[GeoFig], tolerance: i64) -> Option<(usize, usize)> {
    let opi64: (i64, i64) = orig_pos.into();
    geos.iter()
        .enumerate()
        .map(|(bb_idx, bb)| {
            let iter: Box<dyn Iterator<Item = PtI>> = match bb {
                GeoFig::BB(bb) => Box::new(bb.points_iter()),
                GeoFig::Poly(bb) => Box::new(bb.points_iter()),
            };
            let (min_corner_idx, min_corner_dist) = iter
                .map(|c| c.into())
                .map(|c: (i64, i64)| (opi64.0 - c.0).pow(2) + (opi64.1 - c.1).pow(2))
                .enumerate()
                .min_by_key(|x| x.1)
                .unwrap();
            (bb_idx, min_corner_idx, min_corner_dist)
        })
        .filter(|(_, _, c_dist)| c_dist <= &tolerance)
        .min_by_key(|(_, _, c_dist)| *c_dist)
        .map(|(bb_idx, c_idx, _)| (bb_idx, c_idx))
}

pub(super) fn import_coco_if_triggered(
    meta_data: &MetaData,
    coco_file: Option<&CocoFile>,
) -> Option<BboxSpecificData> {
    if let Some(coco_file) = coco_file {
        match tools_data::coco_io::read_coco(meta_data, coco_file) {
            Ok(bbox_data) => Some(bbox_data),
            Err(e) => {
                tracing::error!("could not import coco due to {e:?}");
                None
            }
        }
    } else {
        None
    }
}

pub(super) fn export_if_triggered(meta_data: &MetaData, bbox_data: &BboxSpecificData) {
    if bbox_data.options.is_export_triggered {
        match tools_data::write_coco(meta_data, bbox_data.clone()) {
            Ok(p) => tracing::info!("export to {p:?} successful"),
            Err(e) => tracing::error!("export failed due to {e:?}"),
        }
    }
}

pub(super) struct MouseHeldParams<'a> {
    pub mover: &'a mut Mover,
}
pub(super) fn on_mouse_held_right(
    mouse_pos: Option<PtF>,
    params: MouseHeldParams,
    mut world: World,
    mut history: History,
) -> (World, History) {
    let orig_shape = world.data.shape();
    let mut add_to_history = false;
    let move_boxes = |mpo_from, mpo_to| {
        let split_mode = get_options(&world).map(|o| o.split_mode);
        let annos = get_annos_mut(&mut world);
        if let (Some(annos), Some(split_mode)) = (annos, split_mode) {
            let tmp =
                mem::take(annos).selected_follow_movement(mpo_from, mpo_to, orig_shape, split_mode);
            (*annos, add_to_history) = tmp;
        }
        Some(())
    };
    params.mover.move_mouse_held(move_boxes, mouse_pos);
    if add_to_history {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
    }
    let are_boxes_visible = are_boxes_visible(&world);
    world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    (world, history)
}

#[derive(Clone, Debug, Default)]
pub(super) struct PrevPos {
    pub prev_pos: Vec<PtF>,
    pub last_valid_click: Option<PtF>,
}

pub(super) struct MouseReleaseParams {
    pub prev_pos: PrevPos,

    pub are_boxes_visible: bool,
    pub is_alt_held: bool,
    pub is_shift_held: bool,
    pub is_ctrl_held: bool,
}

pub(super) fn on_mouse_released_right(
    mouse_pos: Option<PtF>,
    mut prev_pos: PrevPos,
    are_boxes_visible: bool,
    mut world: World,
    mut history: History,
) -> (World, History, PrevPos) {
    let split_mode = get_options(&world).map(|o| o.split_mode);
    let lc_orig = prev_pos.last_valid_click;
    let in_menu_selected_label = current_cat_idx(&world);
    if let (Some(mp), Some(last_click), Some(split_mode), Some(in_menu_selected_label)) =
        (mouse_pos, lc_orig, split_mode, in_menu_selected_label)
    {
        match prev_pos.prev_pos.len().cmp(&1) {
            Ordering::Equal => {
                // second click new bb
                let pp = prev_pos.prev_pos[0];
                if (mp.x as i32 - pp.x as i32).abs() > 1 && (mp.y as i32 - pp.y as i32).abs() > 1 {
                    let mp = match split_mode {
                        SplitMode::Horizontal => (last_click.x, mp.y).into(),
                        SplitMode::Vertical => (mp.x, last_click.y).into(),
                        SplitMode::None => mp,
                    };
                    let annos = get_annos_mut(&mut world);
                    if let Some(annos) = annos {
                        annos.add_bb(
                            BB::from_points(mp.into(), pp.into()),
                            in_menu_selected_label,
                        );
                        history.push(Record::new(world.data.clone(), ACTOR_NAME));
                        prev_pos.prev_pos = vec![];
                        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
                    }
                }
            }
            Ordering::Greater => {
                prev_pos.prev_pos.push(mp);
                let poly = Polygon::from_vec(
                    prev_pos
                        .prev_pos
                        .into_iter()
                        .map(|p| p.into())
                        .collect::<Vec<_>>(),
                )
                .unwrap();
                prev_pos.prev_pos = vec![];
                let annos = get_annos_mut(&mut world);
                if let Some(annos) = annos {
                    annos.add_elt(GeoFig::Poly(poly), in_menu_selected_label);
                    history.push(Record::new(world.data.clone(), ACTOR_NAME));
                    prev_pos.prev_pos = vec![];
                    world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
                }
            }
            _ => (),
        }
    }
    (world, history, prev_pos)
}
pub(super) fn on_mouse_released_left(
    mouse_pos: Option<PtF>,
    params: MouseReleaseParams,
    mut world: World,
    mut history: History,
) -> (World, History, PrevPos) {
    let split_mode = get_options(&world).map(|o| o.split_mode);
    let are_annotations_visible = are_boxes_visible(&world);
    let MouseReleaseParams {
        mut prev_pos,
        are_boxes_visible,
        is_alt_held,
        is_shift_held,
        is_ctrl_held,
    } = params;
    let in_menu_selected_label = current_cat_idx(&world);
    if let Some(mp) = mouse_pos {
        prev_pos.last_valid_click = Some(mp);
    }
    if is_alt_held && is_shift_held && !prev_pos.prev_pos.is_empty() {
        // delete the whole thing
        prev_pos.prev_pos = vec![];
        world.request_redraw_annotations(BBOX_NAME, are_annotations_visible);
    } else if is_alt_held && !prev_pos.prev_pos.is_empty() {
        // delete prev pos
        prev_pos.prev_pos.pop();
        if prev_pos.prev_pos.is_empty() {
            world.request_redraw_annotations(BBOX_NAME, are_annotations_visible);
        }
    } else if is_ctrl_held || is_alt_held || is_shift_held {
        // selection
        let annos = get_annos_mut(&mut world);
        if let Some(annos) = annos {
            let idx = mouse_pos.and_then(|p| find_closest_boundary_idx(p, annos.elts()));
            if let Some(i) = idx {
                if is_shift_held {
                    // If shift is held a new selection box will be spanned between the currently clicked
                    // box and the selected box that has the maximum distance in terms of max-corner-dist.
                    // All boxes that have overlap with this new selection box will be selected. If no box
                    // is selected only the currently clicked box will be selected.
                    annos.select(i);
                    let newly_selected_bb = &annos.elts()[i];
                    let sel_indxs = true_indices(annos.selected_mask());
                    if let Some((p1, p2, _)) = sel_indxs
                        .map(|i| (newly_selected_bb.max_squaredist(&annos.elts()[i])))
                        .max_by_key(|(_, _, d)| *d)
                    {
                        let spanned_bb = BB::from_points(p1, p2);
                        let to_be_selected_inds = annos
                            .elts()
                            .iter()
                            .enumerate()
                            .filter(|(_, bb)| bb.has_overlap(&spanned_bb))
                            .map(|(i, _)| i)
                            .collect::<Vec<_>>();
                        annos.select_multi(to_be_selected_inds.iter().copied());
                    }
                } else if is_alt_held {
                    annos.deselect_all();
                    annos.select(i);
                    if let Some(selected) = in_menu_selected_label {
                        annos.label_selected(selected);
                    }
                } else {
                    // ctrl
                    annos.toggle_selection(i);
                }
            }
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        }
    } else {
        let shape_orig = world.data.shape();
        let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
        let tolerance = (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2);
        let close_corner = mouse_pos.and_then(|mp| {
            get_annos_if_some(&world)
                .and_then(|a| find_close_vertex(mp, a.elts(), tolerance as i64))
        });
        if let Some((bb_idx, vertex_idx)) = close_corner {
            // move an existing corner
            let annos = get_annos_mut(&mut world);
            if let Some(annos) = annos {
                let geo = annos.remove(bb_idx);
                match geo {
                    GeoFig::BB(bb) => {
                        let oppo_corner = bb.opposite_corner(vertex_idx);
                        prev_pos.prev_pos.push(oppo_corner.into());
                    }
                    GeoFig::Poly(poly) => {
                        let n_vertices = poly.points().len();
                        prev_pos.prev_pos = vec![];
                        prev_pos.prev_pos.reserve(n_vertices);
                        for idx in (vertex_idx + 1)..(n_vertices) {
                            prev_pos.prev_pos.push(poly.points()[idx].into());
                        }
                        for idx in 0..vertex_idx {
                            prev_pos.prev_pos.push(poly.points()[idx].into());
                        }
                    }
                }
            }
        } else {
            match split_mode {
                Some(SplitMode::None) => {
                    // first click new bb
                    if let Some(mp) = mouse_pos {
                        prev_pos.prev_pos.push(mp);
                    }
                }
                _ => {
                    // create boxes by splitting either horizontally or vertically
                    if let Some(mp) = mouse_pos {
                        let existing_bbs = || -> Box<dyn Iterator<Item = &BB>> {
                            if let Some(annos) = get_annos(&world) {
                                Box::new(annos.elts().iter().flat_map(|geo| match geo {
                                    GeoFig::BB(bb) => Some(bb),
                                    GeoFig::Poly(_) => None,
                                }))
                            } else {
                                Box::new(empty())
                            }
                        };
                        let new_bbs = if let Some(SplitMode::Horizontal) = split_mode {
                            if let Some((i, bb)) = existing_bbs()
                                .enumerate()
                                .find(|(_, bb)| bb.contains((mp.x, mp.y)))
                            {
                                let (top, btm) = bb.split_horizontally(mp.y as u32);
                                vec![(Some(i), top, btm)]
                            } else {
                                let new_bbs = existing_bbs()
                                    .enumerate()
                                    .filter(|(_, bb)| bb.covers_y(mp.y))
                                    .map(|(i, bb)| {
                                        let (top, btm) = bb.split_horizontally(mp.y as u32);
                                        (Some(i), top, btm)
                                    })
                                    .collect::<Vec<_>>();
                                if new_bbs.is_empty() {
                                    let (top, btm) =
                                        BB::from_shape(shape_orig).split_horizontally(mp.y as u32);
                                    vec![(None, top, btm)]
                                } else {
                                    new_bbs
                                }
                            }
                        // SplitMode::Vertical
                        } else if let Some((i, bb)) = existing_bbs()
                            .enumerate()
                            .find(|(_, bb)| bb.contains((mp.x, mp.y)))
                        {
                            let (left, right) = bb.split_vertically(mp.x as u32);
                            vec![(Some(i), left, right)]
                        } else {
                            let new_bbs = existing_bbs()
                                .enumerate()
                                .filter(|(_, bb)| bb.covers_x(mp.x))
                                .map(|(i, bb)| {
                                    let (left, right) = bb.split_vertically(mp.x as u32);
                                    (Some(i), left, right)
                                })
                                .collect::<Vec<_>>();
                            if new_bbs.is_empty() {
                                let (left, right) =
                                    BB::from_shape(shape_orig).split_vertically(mp.x as u32);
                                vec![(None, left, right)]
                            } else {
                                new_bbs
                            }
                        };
                        let annos = get_annos_mut(&mut world);
                        if let Some(annos) = annos {
                            let removers =
                                new_bbs.iter().flat_map(|(i, _, _)| *i).collect::<Vec<_>>();
                            annos.remove_multiple(&removers);
                            if let Some(selected) = in_menu_selected_label {
                                for (_, bb1, bb2) in new_bbs {
                                    annos.add_bb(bb1, selected);
                                    annos.add_bb(bb2, selected);
                                }
                            }
                            history.push(Record::new(world.data.clone(), ACTOR_NAME));
                            prev_pos.prev_pos = vec![];
                            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
                        }
                    }
                }
            }
        }
    }
    (world, history, prev_pos)
}

macro_rules! released_key {
    ($($key:ident),*) => {
        #[derive(Debug, Clone, Copy)]
        pub(super) enum ReleasedKey {
            None,
            $($key,)*
        }
        pub(super) fn map_released_key(event: &Events) -> ReleasedKey {
            if false {
                ReleasedKey::None
            } $(else if event.released(KeyCode::$key) {
                ReleasedKey::$key
            })*
            else {
                ReleasedKey::None
            }
        }
    };
}

released_key!(
    A, D, H, C, V, L, Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9, Delete, Back,
    Left, Right, Up, Down
);

macro_rules! set_cat_current {
    ($num:expr, $world:expr) => {
        let specifics = get_specific_mut(&mut $world);
        if let Some(specifics) = specifics {
            if $num < specifics.label_info.cat_ids().len() + 1 {
                specifics.label_info.cat_idx_current = $num - 1;
            }
        }
    };
}

pub(super) struct KeyReleasedParams {
    pub is_ctrl_held: bool,
    pub released_key: ReleasedKey,
}

pub(super) fn on_key_released(
    mut world: World,
    mut history: History,
    mouse_pos: Option<PtF>,
    params: KeyReleasedParams,
) -> (World, History) {
    let options = get_options(&world);
    if let Some(options) = options {
        match params.released_key {
            ReleasedKey::H if params.is_ctrl_held => {
                // Hide all boxes (selected or not)
                if let Some(options_mut) = get_options_mut(&mut world) {
                    options_mut.are_boxes_visible = !options.are_boxes_visible;
                }
                world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
            }
            ReleasedKey::Delete | ReleasedKey::Back => {
                // Remove selected
                let annos = get_annos_mut(&mut world);
                if let Some(annos) = annos {
                    if !annos.selected_mask().is_empty() {
                        annos.remove_selected();
                        world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
                        history.push(Record::new(world.data.clone(), ACTOR_NAME));
                    }
                }
            }
            ReleasedKey::A if params.is_ctrl_held => {
                // Select all
                if let Some(a) = get_annos_mut(&mut world) {
                    a.select_all()
                };
                world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
            }
            ReleasedKey::D if params.is_ctrl_held => {
                // Deselect all
                if let Some(a) = get_annos_mut(&mut world) {
                    a.deselect_all()
                };
                world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
            }
            ReleasedKey::C if params.is_ctrl_held => {
                // Copy to clipboard
                let clipboard_data = get_annos(&world).map(ClipboardData::from_annotations);
                let clipboard_mut = get_specific_mut(&mut world).map(|d| &mut d.clipboard);
                if let Some(clipboard_mut) = clipboard_mut {
                    *clipboard_mut = clipboard_data;
                }

                world.request_redraw_annotations(BBOX_NAME, options.are_boxes_visible);
            }
            ReleasedKey::V if params.is_ctrl_held => {
                (world, history) = paste(world, history);
            }
            ReleasedKey::V => {
                if let Some(options_mut) = get_options_mut(&mut world) {
                    options_mut.auto_paste = !options.auto_paste;
                }
            }
            ReleasedKey::C => {
                // Paste selection directly at current mouse position
                if let Some((x_shift, y_shift)) = mouse_pos.map(<PtF as Into<(i32, i32)>>::into) {
                    let shape_orig = world.shape_orig();
                    let annos = get_annos_mut(&mut world);
                    if let Some(annos) = annos {
                        let selected_inds = true_indices(annos.selected_mask());
                        let first_selected_idx = true_indices(annos.selected_mask()).next();
                        if let Some(first_idx) = first_selected_idx {
                            let translated = selected_inds.flat_map(|idx| {
                                let geo = annos.elts()[idx].clone();
                                let first = &annos.elts()[first_idx];
                                geo.translate(
                                    Point {
                                        x: x_shift - first.enclosing_bb().min().x as i32,
                                        y: y_shift - first.enclosing_bb().min().y as i32,
                                    },
                                    shape_orig,
                                    OutOfBoundsMode::Deny,
                                )
                                .map(|bb| (bb, annos.cat_idxs()[idx]))
                            });
                            let translated_bbs =
                                translated.clone().map(|(bb, _)| bb).collect::<Vec<_>>();
                            let translated_cat_ids =
                                translated.map(|(_, cat_id)| cat_id).collect::<Vec<_>>();

                            if !translated_bbs.is_empty() {
                                annos.extend(
                                    translated_bbs.iter().cloned(),
                                    translated_cat_ids.iter().copied(),
                                    shape_orig,
                                );
                                annos.deselect_all();
                                annos.select_last_n(translated_bbs.len());
                                world.request_redraw_annotations(
                                    BBOX_NAME,
                                    options.are_boxes_visible,
                                );
                                history.push(Record::new(world.data.clone(), ACTOR_NAME));
                            }
                        }
                    }
                }
            }
            ReleasedKey::Up | ReleasedKey::Down | ReleasedKey::Left | ReleasedKey::Right => {
                history.push(Record::new(world.data.clone(), ACTOR_NAME));
            }
            ReleasedKey::Key1 => {
                set_cat_current!(1, world);
            }
            ReleasedKey::Key2 => {
                set_cat_current!(2, world);
            }
            ReleasedKey::Key3 => {
                set_cat_current!(3, world);
            }
            ReleasedKey::Key4 => {
                set_cat_current!(4, world);
            }
            ReleasedKey::Key5 => {
                set_cat_current!(5, world);
            }
            ReleasedKey::Key6 => {
                set_cat_current!(6, world);
            }
            ReleasedKey::Key7 => {
                set_cat_current!(7, world);
            }
            ReleasedKey::Key8 => {
                set_cat_current!(8, world);
            }
            ReleasedKey::Key9 => {
                set_cat_current!(9, world);
            }
            _ => (),
        }
    }
    (world, history)
}

#[cfg(test)]
use {
    super::core::get_specific,
    crate::{
        domain::{make_test_bbs, make_test_geos, Shape},
        point,
        tools_data::annotations::BboxAnnotations,
        types::ViewImage,
    },
    image::DynamicImage,
    std::collections::HashMap,
};

#[cfg(test)]
fn test_data() -> (Option<PtF>, World, History) {
    let im_test = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let mut world = World::from_real_im(im_test, HashMap::new(), "superimage.png".to_string());
    world.data.meta_data.is_loading_screen_active = Some(false);
    get_specific_mut(&mut world)
        .unwrap()
        .label_info
        .push("label".to_string(), None, None)
        .unwrap();
    let history = History::default();
    let mouse_pos = Some(point!(32.0, 32.0));
    (mouse_pos, world, history)
}

#[cfg(test)]
fn history_equal(hist1: &History, hist2: &History) -> bool {
    format!("{:?}", hist1) == format!("{:?}", hist2)
}

#[test]
fn test_key_released() {
    let (_, mut world, history) = test_data();
    let make_params = |released_key, is_ctrl_held| KeyReleasedParams {
        is_ctrl_held,
        released_key,
    };
    let annos = get_annos_mut(&mut world).unwrap();
    annos.add_bb(
        BB {
            x: 1,
            y: 1,
            h: 10,
            w: 10,
        },
        0,
    );
    assert!(!annos.selected_mask()[0]);
    let annos_orig = annos.clone();

    // select all boxes with ctrl+A
    let params = make_params(ReleasedKey::A, false);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_mask()[0]);
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_mask()[0]);

    // copy and paste boxes to and from clipboard
    let params = make_params(ReleasedKey::C, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_mask()[0]);
    if let Some(clipboard) = get_specific(&world).and_then(|d| d.clipboard.clone()) {
        let mut annos = BboxAnnotations::default();
        annos.extend(
            clipboard.geos().iter().cloned(),
            clipboard.cat_idxs().iter().copied(),
            Shape { w: 100, h: 100 },
        );
        assert_eq!(annos.elts(), get_annos(&world).unwrap().elts());
        assert_eq!(annos.cat_idxs(), get_annos(&world).unwrap().cat_idxs());
        assert_ne!(
            annos.selected_mask(),
            get_annos(&world).unwrap().selected_mask()
        );
    } else {
        assert!(false);
    }
    let params = make_params(ReleasedKey::V, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_specific(&world).unwrap().clipboard.is_some());
    assert_eq!(get_annos(&world).unwrap().elts(), annos_orig.elts());
    let params = make_params(ReleasedKey::C, true);
    let (mut world, history) = on_key_released(world, history, None, params);
    get_annos_mut(&mut world).unwrap().remove(0);
    let params = make_params(ReleasedKey::V, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert_eq!(get_annos(&world).unwrap().elts(), annos_orig.elts());

    // clone box
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::C, false);
    let (world, history) = on_key_released(world, history, Some(point!(2.0, 2.0)), params);
    assert_eq!(get_annos(&world).unwrap().elts()[0], annos_orig.elts()[0]);
    assert_eq!(
        get_annos(&world).unwrap().elts()[1],
        annos_orig.elts()[0]
            .clone()
            .translate(
                Point { x: 1, y: 1 },
                world.shape_orig(),
                OutOfBoundsMode::Deny
            )
            .unwrap()
    );
    assert_eq!(get_annos(&world).unwrap().elts().len(), 2);

    // deselect all boxes with ctrl+D
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::D, false);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_mask()[0]);
    let params = make_params(ReleasedKey::D, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_options(&world).unwrap();
    assert!(flags.are_boxes_visible);
    assert!(!get_annos(&world).unwrap().selected_mask()[0]);

    // hide all boxes with ctrl+H
    let params = make_params(ReleasedKey::H, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_options(&world).unwrap();
    assert!(!flags.are_boxes_visible);

    // delete all selected boxes with ctrl+Delete
    let params = make_params(ReleasedKey::Delete, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_mask().is_empty());
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::Delete, true);
    let (world, _) = on_key_released(world, history, None, params);
    assert!(get_annos(&world).unwrap().selected_mask().is_empty());
}

#[test]
fn test_mouse_held() {
    let (mouse_pos, mut world, history) = test_data();
    let annos = get_annos_mut(&mut world);
    let bbs = make_test_bbs();
    annos.unwrap().add_bb(bbs[0].clone(), 0);
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some(point!(12.0, 12.0)));
        let params = MouseHeldParams { mover: &mut mover };
        let (world, new_hist) =
            on_mouse_held_right(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(get_annos(&world).unwrap().elts()[0], GeoFig::BB(bbs[0]));
        assert!(history_equal(&history, &new_hist));
    }
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some(point!(12.0, 12.0)));
        let params = MouseHeldParams { mover: &mut mover };
        let annos = get_annos_mut(&mut world);
        annos.unwrap().select(0);
        let (world, new_hist) = on_mouse_held_right(mouse_pos, params, world, history.clone());
        assert_ne!(get_annos(&world).unwrap().elts()[0], GeoFig::BB(bbs[0]));

        assert!(!history_equal(&history, &new_hist));
    }
}

#[test]
fn test_mouse_release() {
    let (mouse_pos, world, history) = test_data();
    let make_params = |prev_pos: Vec<PtF>, is_ctrl_held| {
        let is_pp_empty = prev_pos.is_empty();
        let last = prev_pos.iter().last().map(|last| last.clone());
        MouseReleaseParams {
            prev_pos: PrevPos {
                prev_pos,
                last_valid_click: if is_pp_empty { None } else { last },
            },
            are_boxes_visible: true,
            is_alt_held: false,
            is_shift_held: false,
            is_ctrl_held,
        }
    };
    {
        // If a previous position was registered, we expect that the second click creates the
        // bounding box.
        let params = make_params(vec![point!(30.0, 30.0)], false);
        let (world, new_hist, prev_pos) = on_mouse_released_right(
            mouse_pos,
            params.prev_pos,
            params.are_boxes_visible,
            world.clone(),
            history.clone(),
        );
        assert!(prev_pos.prev_pos.is_empty());
        let annos = get_annos(&world);
        assert_eq!(annos.unwrap().elts().len(), 1);
        assert_eq!(annos.unwrap().cat_idxs()[0], 0);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
    {
        // If no position was registered, a left click will trigger the start
        // of defining a new bounding box. The other corner will be defined by a second click.
        let params = make_params(vec![], false);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(
            prev_pos.prev_pos,
            if let Some(mp) = mouse_pos {
                vec![mp]
            } else {
                vec![]
            }
        );
        let annos = get_annos(&world);
        assert!(annos.is_none() || annos.unwrap().elts().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold, a bounding box would be selected. Since no bounding boxes exist,
        // nothing should happen.
        let params = make_params(vec![], true);
        let (world, new_hist, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(prev_pos.prev_pos, vec![]);
        let annos = get_annos(&world);
        assert!(annos.is_none() || annos.unwrap().elts().is_empty());
        assert!(history_equal(&history, &new_hist));
    }
    {
        // If ctrl is hold at the second click, this does not really make sense. We ignore it and assume this
        // is the finishing box click.
        let params = make_params(vec![point!(30.0, 30.0)], true);
        let (world, new_hist, prev_pos) = on_mouse_released_right(
            mouse_pos,
            params.prev_pos,
            params.are_boxes_visible,
            world.clone(),
            history.clone(),
        );
        assert_eq!(prev_pos.prev_pos, vec![]);
        let annos = get_annos(&world);
        assert_eq!(annos.unwrap().elts().len(), 1);
        assert!(!annos.unwrap().selected_mask()[0]);
        assert!(format!("{:?}", new_hist).len() > format!("{:?}", history).len());
    }
    {
        // If ctrl is hold the box is selected.
        let params = make_params(vec![], true);
        let mut world = world.clone();
        get_specific_mut(&mut world)
            .unwrap()
            .label_info
            .push("label2".to_string(), None, None)
            .unwrap();
        get_specific_mut(&mut world)
            .unwrap()
            .label_info
            .cat_idx_current = 1;
        let annos = get_annos_mut(&mut world).unwrap();
        annos.add_bb(BB::from_arr(&[20, 20, 20, 20]), 0);
        annos.add_bb(BB::from_arr(&[50, 50, 5, 5]), 0);
        annos.add_bb(BB::from_arr(&[20, 50, 3, 3]), 1);
        annos.add_bb(BB::from_arr(&[20, 55, 3, 3]), 0);

        let (mut world, _, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, vec![]);
        assert!(annos.selected_mask()[0]);
        assert!(!annos.selected_mask()[1]);
        assert_eq!(annos.cat_idxs()[0], 0);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
        // alt
        get_specific_mut(&mut world)
            .unwrap()
            .label_info
            .cat_idx_current = 1;
        let mut params = make_params(vec![], true);
        params.is_alt_held = true;
        let annos = get_annos_mut(&mut world).unwrap();
        annos.deselect_all();
        annos.select(1);
        let (mut world, _, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, vec![]);
        assert!(annos.selected_mask()[0]);
        assert!(!annos.selected_mask()[1]);
        assert_eq!(annos.cat_idxs()[0], 1);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
        // shift
        let mut params = make_params(vec![], true);
        params.is_shift_held = true;
        let annos = get_annos_mut(&mut world).unwrap();
        annos.select(3);
        let (world, _, prev_pos) =
            on_mouse_released_left(mouse_pos, params, world.clone(), history.clone());
        let annos = get_annos(&world).unwrap();
        assert_eq!(prev_pos.prev_pos, vec![]);
        assert!(annos.selected_mask()[0]);
        assert!(!annos.selected_mask()[1]);
        assert!(annos.selected_mask()[2]);
        assert!(annos.selected_mask()[3]);
        assert_eq!(annos.cat_idxs()[0], 1);
        assert_eq!(annos.cat_idxs()[1], 0);
        assert_eq!(annos.cat_idxs()[2], 1);
        assert_eq!(annos.cat_idxs()[3], 0);
    }
}

#[test]
fn test_find_idx() {
    let bbs = make_test_geos();
    assert_eq!(find_closest_boundary_idx((0.0, 20.0).into(), &bbs), None);
    assert_eq!(find_closest_boundary_idx((0.0, 0.0).into(), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((3.0, 8.0).into(), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((7.0, 14.0).into(), &bbs), Some(1));
    assert_eq!(find_closest_boundary_idx((7.0, 15.0).into(), &bbs), None);
    assert_eq!(find_closest_boundary_idx((8.0, 8.0).into(), &bbs), Some(0));
    assert_eq!(
        find_closest_boundary_idx((10.0, 12.0).into(), &bbs),
        Some(2)
    );
}
