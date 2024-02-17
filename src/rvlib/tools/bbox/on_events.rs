use std::{cmp::Ordering, iter, iter::empty, mem};

use lazy_static::lazy_static;

use crate::{
    cfg::ExportPath,
    domain::{
        self, max_from_partial, min_from_partial, shape_unscaled, BbF, InstanceAnnotate,
        OutOfBoundsMode, Point, PtF, ShapeF, TPtF,
    },
    file_util::MetaData,
    history::{History, Record},
    result::{trace_ok, RvResult},
    tools::{
        core::{
            change_annos, label_change_key, make_track_changes_str, on_selection_keys, Mover,
            ReleasedKey,
        },
        BBOX_NAME,
    },
    tools_data::{
        self,
        annotations::{BboxAnnotations, SplitMode},
        BboxSpecificData, Rot90ToolData,
    },
    util::{true_indices, Visibility},
    world::World,
    GeoFig, Polygon,
};

use super::core::{
    current_cat_idx, get_annos, get_annos_if_some, get_annos_mut, get_label_info, get_options,
    get_options_mut, get_specific, get_specific_mut, get_visible, ACTOR_NAME,
};

const CORNER_TOL_DENOMINATOR: f64 = 5000.0;

pub(super) fn change_annos_bbox(world: &mut World, change: impl FnOnce(&mut BboxAnnotations)) {
    lazy_static! {
        static ref TRACK_CHANGE_STR: String = make_track_changes_str(ACTOR_NAME);
    };
    let track_changes = get_options(world).map(|o| o.core_options.track_changes) == Some(true);
    change_annos(
        world,
        TRACK_CHANGE_STR.as_str(),
        track_changes,
        change,
        get_annos_mut,
    );
}

fn closest_containing_boundary_idx(
    pos: PtF,
    geos: &[GeoFig],
    predicate: impl Fn(usize) -> bool,
) -> Option<usize> {
    geos.iter()
        .enumerate()
        .filter(|(i, geo)| geo.contains(pos) && predicate(*i))
        .map(|(i, geo)| (i, geo.dist_to_boundary(pos)))
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
        .map(|(i, _)| i)
}

pub(super) fn move_corner_tol(unscaled: ShapeF) -> TPtF {
    (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2.0)
}
pub(super) fn closest_corner(pos: PtF, corners: impl Iterator<Item = PtF>) -> (usize, TPtF) {
    corners
        .map(|c| (pos.x - c.x).powi(2) + (pos.y - c.y).powi(2))
        .enumerate()
        .min_by(|(_, x1), (_, x2)| min_from_partial(x1, x2))
        .unwrap()
}
/// returns index of the bounding box and the index of the closest close corner
pub(super) fn find_close_vertex<'a>(
    orig_pos: PtF,
    geos: impl Iterator<Item = (usize, &'a GeoFig)>,
    tolerance: TPtF,
) -> Option<(usize, usize)> {
    geos.map(|(bb_idx, bb)| {
        let iter: Box<dyn Iterator<Item = PtF>> = match bb {
            GeoFig::BB(bb) => Box::new(bb.points_iter()),
            GeoFig::Poly(poly) => Box::new(poly.points_iter()),
        };
        let (min_corner_idx, min_corner_dist) = closest_corner(orig_pos, iter);
        (bb_idx, min_corner_idx, min_corner_dist)
    })
    .filter(|(_, _, c_dist)| c_dist <= &tolerance)
    .min_by(|(_, _, c_dist_1), (_, _, c_dist_2)| min_from_partial(c_dist_1, c_dist_2))
    .map(|(bb_idx, c_idx, _)| (bb_idx, c_idx))
}

pub(super) fn import_coco_if_triggered(
    meta_data: &MetaData,
    coco_file: Option<&ExportPath>,
    rot90_data: Option<&Rot90ToolData>,
) -> Option<BboxSpecificData> {
    if let Some(coco_file) = coco_file {
        match tools_data::coco_io::read_coco(meta_data, coco_file, rot90_data) {
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

pub(super) fn export_if_triggered(
    meta_data: &MetaData,
    bbox_data: &BboxSpecificData,
    rot90_data: Option<&Rot90ToolData>,
) {
    if bbox_data.options.core_options.is_export_triggered {
        match tools_data::write_coco(meta_data, bbox_data.clone(), rot90_data) {
            Ok(p) => tracing::info!("export to {p:?} successful"),
            Err(e) => tracing::error!("export failed due to {e:?}"),
        }
    }
}

fn shorter_path(
    close_point_idx: usize,
    move_corner_idx: usize,
    points: Vec<PtF>,
) -> RvResult<Vec<PtF>> {
    if let Some(mp) = points.last().cloned() {
        if close_point_idx < move_corner_idx && move_corner_idx < points.len() {
            let path_forward = points[move_corner_idx..]
                .iter()
                .chain(points[close_point_idx..move_corner_idx].iter());
            let path_backward = points[move_corner_idx..]
                .iter()
                .rev()
                .chain(points[..close_point_idx].iter());

            let path_to_closest = points[..close_point_idx + 1].iter();
            let path_from_closest = points[close_point_idx..].iter();
            let length_to_closest = path_to_closest
                .clone()
                .zip(path_to_closest.clone().skip(1))
                .map(|(p1, p2)| p1.dist_square(p2).sqrt())
                .sum::<TPtF>();
            let length_from_closest = path_from_closest
                .clone()
                .zip(path_from_closest.clone().skip(1))
                .map(|(p1, p2)| p1.dist_square(p2).sqrt())
                .sum::<TPtF>()
                + mp.dist_square(&points[0]).sqrt();
            if length_from_closest > length_to_closest {
                Ok(path_forward.cloned().collect::<Vec<_>>())
            } else {
                Ok(iter::once(mp)
                    .chain(path_backward.cloned())
                    .collect::<Vec<_>>())
            }
        } else {
            Ok(points)
        }
    } else {
        Err("points must not be empty".into())
    }
}

fn close_polygon(
    mut prev_pos: PrevPos,
    in_menu_selected_label: usize,
    visible: Visibility,
    mut world: World,
    mut history: History,
) -> Option<(World, History, PrevPos)> {
    if prev_pos.prev_pos.len() > 2 {
        let (c_idx, c_dist) = closest_corner(
            prev_pos.prev_pos.last().cloned()?,
            prev_pos
                .prev_pos
                .iter()
                .take(prev_pos.prev_pos.len() - 1)
                .cloned(),
        );
        let unscaled = shape_unscaled(world.zoom_box(), world.shape_orig());
        let tolerance = move_corner_tol(unscaled);
        let poly = if c_dist < tolerance {
            if let Some(mc_idx) = prev_pos.move_corner_idx {
                trace_ok(Polygon::from_vec(trace_ok(shorter_path(
                    c_idx,
                    mc_idx,
                    prev_pos.prev_pos,
                ))?))?
            } else {
                trace_ok(Polygon::from_vec(
                    prev_pos.prev_pos.into_iter().collect::<Vec<_>>(),
                ))?
            }
        } else {
            trace_ok(Polygon::from_vec(
                prev_pos.prev_pos.into_iter().collect::<Vec<_>>(),
            ))?
        };
        prev_pos.prev_pos = vec![];
        let add_annos = |annos: &mut BboxAnnotations| {
            annos.add_elt(GeoFig::Poly(poly), in_menu_selected_label);
        };
        change_annos_bbox(&mut world, add_annos);
        history.push(Record::new(world.clone(), ACTOR_NAME));
        prev_pos.prev_pos = vec![];
        world.request_redraw_annotations(BBOX_NAME, visible);
    } else {
        tracing::error!("polygon needs at least 3 points");
    }
    Some((world, history, prev_pos))
}
pub(super) struct MouseMoveParams<'a> {
    pub mover: &'a mut Mover,
}
pub(super) fn on_mouse_held_right(
    mouse_pos: Option<PtF>,
    params: MouseMoveParams,
    mut world: World,
    history: History,
) -> (World, History) {
    if get_options(&world).map(|o| o.core_options.erase) != Some(true) {
        let orig_shape = world.data.shape();
        let move_boxes = |mpo_from, mpo_to| {
            let split_mode = get_options(&world).map(|o| o.split_mode);
            let annos = get_annos_mut(&mut world);
            if let (Some(annos), Some(split_mode)) = (annos, split_mode) {
                let tmp = mem::take(annos)
                    .selected_follow_movement(mpo_from, mpo_to, orig_shape, split_mode);
                (*annos, _) = tmp;
            }
            Some(())
        };
        params.mover.move_mouse_held(move_boxes, mouse_pos);
        let vis = get_visible(&world);
        world.request_redraw_annotations(BBOX_NAME, vis);
    }
    (world, history)
}

#[derive(Clone, Debug, Default)]
pub(super) struct PrevPos {
    pub prev_pos: Vec<PtF>,
    pub last_valid_click: Option<PtF>,
    pub move_corner_idx: Option<usize>,
}

pub(super) struct MouseReleaseParams {
    pub prev_pos: PrevPos,
    pub visible: Visibility,
    pub is_alt_held: bool,
    pub is_shift_held: bool,
    pub is_ctrl_held: bool,
    pub close_box_or_poly: bool,
}
pub(super) struct MouseHeldLeftParams {
    pub prev_pos: PrevPos,
    pub is_alt_held: bool,
    pub is_shift_held: bool,
    pub is_ctrl_held: bool,
    pub distance: f64,
    pub elapsed_millis_since_press: u128,
}

pub(super) fn on_mouse_released_right(
    mouse_pos: Option<PtF>,
    mut prev_pos: PrevPos,
    visible: Visibility,
    mut world: World,
    mut history: History,
) -> (World, History, PrevPos) {
    if get_options(&world).map(|o| o.core_options.erase) != Some(true) {
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
                    if (mp.x - pp.x).abs() > 1.0 && (mp.y - pp.y).abs() > 1.0 {
                        let mp = match split_mode {
                            SplitMode::Horizontal => (last_click.x, mp.y).into(),
                            SplitMode::Vertical => (mp.x, last_click.y).into(),
                            SplitMode::None => mp,
                        };
                        let add_annos = |annos: &mut BboxAnnotations| {
                            annos.add_bb(BbF::from_points(mp, pp), in_menu_selected_label);
                        };
                        change_annos_bbox(&mut world, add_annos);
                        history.push(Record::new(world.clone(), ACTOR_NAME));
                        prev_pos.prev_pos = vec![];
                        world.request_redraw_annotations(BBOX_NAME, visible);
                    }
                }
                Ordering::Greater => {
                    prev_pos.prev_pos.push(mp);
                    (world, history, prev_pos) =
                        close_polygon(prev_pos, in_menu_selected_label, visible, world, history)
                            .unwrap();
                }
                _ => (),
            }
        }
        if mouse_pos.is_some() {
            let annos = get_annos(&world);
            if let Some(annos) = annos {
                if (0..annos.selected_mask().len()).any(|i| annos.selected_mask()[i]) {
                    history.push(Record::new(world.clone(), ACTOR_NAME));
                }
            }
        }
    }
    (world, history, prev_pos)
}

pub(super) fn on_mouse_held_left(
    mouse_pos: Option<PtF>,
    mut params: MouseHeldLeftParams,
    world: World,
    history: History,
) -> (World, History, PrevPos) {
    if params.elapsed_millis_since_press > 200
        && get_options(&world).map(|o| o.core_options.erase) != Some(true)
    {
        const SENSITIVITY_FACTOR: f64 = 5.0;
        let min_distance_start_end = (SENSITIVITY_FACTOR * params.distance).max(5.0);
        if !(params.is_alt_held || params.is_ctrl_held || params.is_shift_held) {
            let pp = &params.prev_pos.prev_pos;
            if let (Some(mp), Some(last_pp), Some(first_pp)) = (mouse_pos, pp.last(), pp.first()) {
                let last_dist = mp.dist_square(last_pp).sqrt();
                let n_pp = pp.len();
                if n_pp == 1 && last_dist > min_distance_start_end {
                    params.prev_pos.prev_pos.push(mp);
                } else if n_pp > 1
                    && last_dist > min_distance_start_end
                    && first_pp.dist_square(&mp).sqrt() > min_distance_start_end
                {
                    let ls = (pp[n_pp - 2], pp[n_pp - 1]);
                    let dist_to_ls = domain::dist_lineseg_point(&ls, mp);
                    if last_dist * 0.2 + dist_to_ls * 0.8 > params.distance {
                        params.prev_pos.prev_pos.push(mp);
                    }
                }
            } else if let Some(mp) = mouse_pos {
                params.prev_pos.prev_pos.push(mp);
            }
        }
    }
    (world, history, params.prev_pos)
}
pub(super) fn on_mouse_released_left(
    mouse_pos: Option<PtF>,
    params: MouseReleaseParams,
    mut world: World,
    mut history: History,
) -> (World, History, PrevPos) {
    let split_mode = get_options(&world).map(|o| o.split_mode);
    let MouseReleaseParams {
        mut prev_pos,
        visible,
        is_alt_held,
        is_shift_held,
        is_ctrl_held,
        close_box_or_poly: close,
    } = params;
    let erase = get_options(&world).map(|o| o.core_options.erase);
    let show_only_current = get_specific(&world).map(|d| d.label_info.show_only_current);
    let cat_idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
    if erase == Some(true) {
        let annos = get_annos_mut(&mut world);
        if let Some(annos) = annos {
            let idx = mouse_pos.and_then(|p| {
                closest_containing_boundary_idx(p, annos.elts(), |idx| {
                    annos.is_of_current_label(idx, cat_idx_current, show_only_current)
                })
            });
            let remove_anno = |annos: &mut BboxAnnotations| {
                if let Some(i) = idx {
                    annos.remove(i);
                }
            };
            change_annos_bbox(&mut world, remove_anno);
            if idx.is_some() {
                history.push(Record::new(world.clone(), ACTOR_NAME));
                world.request_redraw_annotations(BBOX_NAME, visible);
            }
        }
        (world, history, prev_pos)
    } else if close {
        let in_menu_selected_label = current_cat_idx(&world);
        if let Some(in_menu_selected_label) = in_menu_selected_label {
            close_polygon(prev_pos, in_menu_selected_label, visible, world, history).unwrap()
        } else {
            (world, history, prev_pos)
        }
    } else {
        let in_menu_selected_label = current_cat_idx(&world);
        if let Some(mp) = mouse_pos {
            prev_pos.last_valid_click = Some(mp);
        }
        if is_alt_held && is_shift_held && !prev_pos.prev_pos.is_empty() {
            // delete the whole thing
            prev_pos.prev_pos = vec![];
            let vis = get_visible(&world);
            world.request_redraw_annotations(BBOX_NAME, vis);
        } else if is_alt_held && !prev_pos.prev_pos.is_empty() {
            // delete prev pos
            prev_pos.prev_pos.pop();
            if prev_pos.prev_pos.is_empty() {
                let vis = get_visible(&world);
                world.request_redraw_annotations(BBOX_NAME, vis);
            }
        } else if is_ctrl_held || is_alt_held || is_shift_held {
            // selection
            let show_only_current = get_specific(&world).map(|d| d.label_info.show_only_current);
            let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
            let annos = get_annos_mut(&mut world);
            if let Some(annos) = annos {
                let idx = mouse_pos.and_then(|p| {
                    closest_containing_boundary_idx(p, annos.elts(), |idx| {
                        annos.is_of_current_label(idx, idx_current, show_only_current)
                    })
                });
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
                            .max_by(|(_, _, d1), (_, _, d2)| max_from_partial(d1, d2))
                        {
                            let spanned_bb = BbF::from_points(p1, p2);
                            let to_be_selected_inds = annos
                                .elts()
                                .iter()
                                .enumerate()
                                .filter(|(elt_idx, bb)| {
                                    annos.is_of_current_label(
                                        *elt_idx,
                                        cat_idx_current,
                                        show_only_current,
                                    ) && bb.has_overlap(&spanned_bb)
                                })
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
                world.request_redraw_annotations(BBOX_NAME, visible);
            }
        } else {
            let shape_orig = world.data.shape();
            let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
            let close_corner = mouse_pos.and_then(|mp| {
                get_annos_if_some(&world).and_then(|a| {
                    find_close_vertex(mp, a.elts().iter().enumerate(), move_corner_tol(unscaled))
                })
            });
            if let Some((bb_idx, vertex_idx)) = close_corner {
                // move an existing corner
                let annos = get_annos_mut(&mut world);
                if let Some(annos) = annos {
                    if annos.is_of_current_label(bb_idx, cat_idx_current, show_only_current) {
                        let geo = annos.remove(bb_idx);
                        match geo {
                            GeoFig::BB(bb) => {
                                let oppo_corner = bb.opposite_corner(vertex_idx);
                                prev_pos.prev_pos.push(oppo_corner);
                            }
                            GeoFig::Poly(poly) => {
                                let n_vertices = poly.points().len();
                                prev_pos.prev_pos = vec![];
                                prev_pos.prev_pos.reserve(n_vertices);
                                for idx in (vertex_idx + 1)..(n_vertices) {
                                    prev_pos.prev_pos.push(poly.points()[idx]);
                                }
                                for idx in 0..vertex_idx {
                                    prev_pos.prev_pos.push(poly.points()[idx]);
                                }
                            }
                        }
                        prev_pos.move_corner_idx = Some(prev_pos.prev_pos.len() - 1);
                    }
                }
            } else {
                match split_mode {
                    Some(SplitMode::None) => {
                        // add point to box/polygon
                        if let Some(mp) = mouse_pos {
                            prev_pos.prev_pos.push(mp);
                        }
                    }
                    _ => {
                        // create boxes by splitting either horizontally or vertically
                        if let Some(mp) = mouse_pos {
                            let existing_bbs = || -> Box<dyn Iterator<Item = &BbF>> {
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
                                    let (top, btm) = bb.split_horizontally(mp.y);
                                    vec![(Some(i), top, btm)]
                                } else {
                                    let new_bbs = existing_bbs()
                                        .enumerate()
                                        .filter(|(_, bb)| bb.covers_y(mp.y))
                                        .map(|(i, bb)| {
                                            let (top, btm) = bb.split_horizontally(mp.y);
                                            (Some(i), top, btm)
                                        })
                                        .collect::<Vec<_>>();
                                    if new_bbs.is_empty() {
                                        let (top, btm) = BbF::from_shape_int(shape_orig)
                                            .split_horizontally(mp.y);
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
                                let (left, right) = bb.split_vertically(mp.x);
                                vec![(Some(i), left, right)]
                            } else {
                                let new_bbs = existing_bbs()
                                    .enumerate()
                                    .filter(|(_, bb)| bb.covers_x(mp.x))
                                    .map(|(i, bb)| {
                                        let (left, right) = bb.split_vertically(mp.x);
                                        (Some(i), left, right)
                                    })
                                    .collect::<Vec<_>>();
                                if new_bbs.is_empty() {
                                    let (left, right) =
                                        BbF::from_shape_int(shape_orig).split_vertically(mp.x);
                                    vec![(None, left, right)]
                                } else {
                                    new_bbs
                                }
                            };
                            let split_annos = |annos: &mut BboxAnnotations| {
                                let removers =
                                    new_bbs.iter().flat_map(|(i, _, _)| *i).collect::<Vec<_>>();
                                annos.remove_multiple(&removers);
                                if let Some(selected) = in_menu_selected_label {
                                    for (_, bb1, bb2) in new_bbs {
                                        annos.add_bb(bb1, selected);
                                        annos.add_bb(bb2, selected);
                                    }
                                }
                            };
                            change_annos_bbox(&mut world, split_annos);
                            history.push(Record::new(world.clone(), ACTOR_NAME));
                            prev_pos.prev_pos = vec![];
                            world.request_redraw_annotations(BBOX_NAME, visible);
                        }
                    }
                }
            }
        }
        (world, history, prev_pos)
    }
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
    let mut trigger_redraw = false;
    if let Some(label_info) = get_specific_mut(&mut world).map(|s| &mut s.label_info) {
        (*label_info, trigger_redraw) =
            label_change_key(params.released_key, mem::take(label_info));
    }
    if trigger_redraw {
        let vis = get_visible(&world);
        world.request_redraw_annotations(BBOX_NAME, vis);
    }
    (world, history) = on_selection_keys(
        world,
        history,
        params.released_key,
        params.is_ctrl_held,
        BBOX_NAME,
        get_annos_mut,
        |world| get_specific_mut(world).map(|d| &mut d.clipboard),
        |world| get_options(world).map(|o| o.core_options),
        get_label_info,
    );
    match params.released_key {
        ReleasedKey::H if params.is_ctrl_held => {
            // Hide all boxes (selected or not)
            if let Some(options_mut) = get_options_mut(&mut world) {
                options_mut.core_options.visible = !options_mut.core_options.visible;
            }
            let vis = get_visible(&world);
            world.request_redraw_annotations(BBOX_NAME, vis);
        }
        ReleasedKey::V if !params.is_ctrl_held => {
            if let Some(options_mut) = get_options_mut(&mut world) {
                options_mut.auto_paste = !options_mut.auto_paste;
            }
        }
        ReleasedKey::C if !params.is_ctrl_held => {
            // Paste selection directly at current mouse position
            if let Some(Point {
                x: x_shift,
                y: y_shift,
            }) = mouse_pos
            {
                let shape_orig = world.shape_orig();
                let add_anno = |annos: &mut BboxAnnotations| {
                    let selected_inds = true_indices(annos.selected_mask());
                    let first_selected_idx = true_indices(annos.selected_mask()).next();
                    if let Some(first_idx) = first_selected_idx {
                        let translated = selected_inds.flat_map(|idx| {
                            let geo = annos.elts()[idx].clone();
                            let first = &annos.elts()[first_idx];
                            geo.translate(
                                Point {
                                    x: x_shift - first.enclosing_bb().min().x,
                                    y: y_shift - first.enclosing_bb().min().y,
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
                        }
                    }
                };
                change_annos_bbox(&mut world, add_anno);
                let vis = get_visible(&world);
                world.request_redraw_annotations(BBOX_NAME, vis);
                history.push(Record::new(world.clone(), ACTOR_NAME));
            }
        }
        ReleasedKey::Up | ReleasedKey::Down | ReleasedKey::Left | ReleasedKey::Right => {
            history.push(Record::new(world.clone(), ACTOR_NAME));
        }
        _ => (),
    }
    (world, history)
}

#[cfg(test)]
use {
    crate::{
        domain::{make_test_bbs, make_test_geos, BbI, ShapeI},
        point,
        types::ViewImage,
    },
    image::DynamicImage,
    std::collections::HashMap,
};

#[cfg(test)]
pub(super) fn test_data() -> (Option<PtF>, World, History) {
    let im_test = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let mut world = World::from_real_im(
        im_test,
        HashMap::new(),
        Some("superimage.png".to_string()),
        Some(0),
    );
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
        BbF {
            x: 1.0,
            y: 1.0,
            h: 10.0,
            w: 10.0,
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
            clipboard.elts().iter().cloned(),
            clipboard.cat_idxs().iter().copied(),
            ShapeI { w: 100, h: 100 },
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
                Point { x: 1.0, y: 1.0 },
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
    assert!(flags.core_options.visible);
    assert!(!get_annos(&world).unwrap().selected_mask()[0]);

    // hide all boxes with ctrl+H
    let params = make_params(ReleasedKey::H, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_options(&world).unwrap();
    assert!(!flags.core_options.visible);

    // don't delete any box since they are hidden boxes with ctrl+Delete
    let params = make_params(ReleasedKey::Delete, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_mask().is_empty());
    let params = make_params(ReleasedKey::A, true);
    let (world, history) = on_key_released(world, history, None, params);
    let params = make_params(ReleasedKey::Delete, true);
    let (world, history) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_mask().is_empty());

    // un-hide all boxes with ctrl+H
    let params = make_params(ReleasedKey::H, true);
    let (world, history) = on_key_released(world, history, None, params);
    let flags = get_options(&world).unwrap();
    assert!(flags.core_options.visible);
    // and now delete them
    let params = make_params(ReleasedKey::Delete, true);
    let (world, _) = on_key_released(world, history, None, params);
    assert!(!get_annos(&world).unwrap().selected_mask().is_empty());
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
        let params = MouseMoveParams { mover: &mut mover };
        let (world, new_hist) =
            on_mouse_held_right(mouse_pos, params, world.clone(), history.clone());
        assert_eq!(get_annos(&world).unwrap().elts()[0], GeoFig::BB(bbs[0]));
        assert!(history_equal(&history, &new_hist));
    }
    {
        let mut mover = Mover::new();
        mover.move_mouse_pressed(Some(point!(12.0, 12.0)));
        let params = MouseMoveParams { mover: &mut mover };
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
                move_corner_idx: None,
            },
            visible: Visibility::All,
            is_alt_held: false,
            is_shift_held: false,
            is_ctrl_held,
            close_box_or_poly: false,
        }
    };
    {
        // If a previous position was registered, we expect that the second click creates the
        // bounding box.
        let params = make_params(vec![point!(30.0, 30.0)], false);
        let (world, new_hist, prev_pos) = on_mouse_released_right(
            mouse_pos,
            params.prev_pos,
            params.visible,
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
            params.visible,
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
        annos.add_bb(BbI::from_arr(&[20, 20, 20, 20]).into(), 0);
        annos.add_bb(BbI::from_arr(&[50, 50, 5, 5]).into(), 0);
        annos.add_bb(BbI::from_arr(&[20, 50, 3, 3]).into(), 1);
        annos.add_bb(BbI::from_arr(&[20, 55, 3, 3]).into(), 0);

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
    assert_eq!(
        closest_containing_boundary_idx((0.0, 20.0).into(), &bbs, |_| true),
        None
    );
    assert_eq!(
        closest_containing_boundary_idx((0.0, 0.0).into(), &bbs, |_| true),
        Some(0)
    );
    assert_eq!(
        closest_containing_boundary_idx((3.0, 8.0).into(), &bbs, |_| true),
        Some(0)
    );
    assert_eq!(
        closest_containing_boundary_idx((7.0, 14.0).into(), &bbs, |_| true),
        Some(1)
    );
    assert_eq!(
        closest_containing_boundary_idx((7.0, 15.0).into(), &bbs, |_| true),
        Some(1)
    );
    assert_eq!(
        closest_containing_boundary_idx((7.0, 15.1).into(), &bbs, |_| true),
        None
    );
    assert_eq!(
        closest_containing_boundary_idx((8.0, 8.0).into(), &bbs, |_| true),
        Some(0)
    );
    assert_eq!(
        closest_containing_boundary_idx((10.0, 12.0).into(), &bbs, |_| true),
        Some(2)
    );
}
