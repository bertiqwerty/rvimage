use crate::{
    types::ViewImage,
    util::{self, Shape, BB},
};
use image::Rgb;
use rusttype::{Font, Scale};
use std::mem;
const BBOX_ALPHA: u8 = 90;
const BBOX_ALPHA_SELECTED: u8 = 170;

fn resize_bbs(
    mut bbs: Vec<BB>,
    selected_bbs: &[bool],
    x_shift: i32,
    y_shift: i32,
    shape_orig: Shape,
) -> Vec<BB> {
    let selected_idxs = selected_bbs
        .iter()
        .enumerate()
        .filter(|(_, x)| **x)
        .map(|(i, _)| i);
    for idx in selected_idxs {
        if let Some(bb) = bbs[idx].extend_max(x_shift, y_shift, shape_orig) {
            bbs[idx] = bb;
        }
    }
    bbs
}

struct Cats<'a> {
    cat_ids: &'a [usize],
    labels: &'a [String],
    colors: &'a [[u8; 3]],
}
impl<'a> Cats<'a> {
    pub fn color_of_box(&self, box_idx: usize) -> &'a [u8; 3] {
        &self.colors[self.cat_ids[box_idx]]
    }
    pub fn label_of_box(&self, box_idx: usize) -> &'a str {
        self.labels[self.cat_ids[box_idx]].as_str()
    }
}
fn draw_bbs<'a>(
    mut im: ViewImage,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    bbs: &'a [BB],
    selected_bbs: &'a [bool],
    cats: Cats<'a>,
) -> ViewImage {
    let font_data: &[u8] = include_bytes!("../../../Roboto/Roboto-Bold.ttf");
    // remove those box ids that are outside of the zoom box
    let relevant_box_inds = (0..bbs.len()).filter(|box_idx| {
        if let Some(zb) = zoom_box {
            !bbs[*box_idx].y + bbs[*box_idx].h < zb.y
                || bbs[*box_idx].x + bbs[*box_idx].w < zb.x
                || bbs[*box_idx].x > zb.x + zb.w
                || bbs[*box_idx].y > zb.y + zb.h
        } else {
            true
        }
    });
    for box_idx in relevant_box_inds {
        let alpha = if selected_bbs[box_idx] {
            BBOX_ALPHA_SELECTED
        } else {
            BBOX_ALPHA
        };
        let f_inner_color =
            |rgb: &Rgb<u8>| util::apply_alpha(&rgb.0, cats.color_of_box(box_idx), alpha);
        let view_corners = bbs[box_idx].to_view_corners(shape_orig, shape_win, zoom_box);

        let color_rgb = Rgb(*cats.color_of_box(box_idx));
        im = util::draw_bx_on_image(
            im,
            view_corners.0,
            view_corners.1,
            &color_rgb,
            f_inner_color,
        );

        // draw label field
        // we do not the label field for the empty-string-label
        if !cats.label_of_box(box_idx).is_empty() {
            if let ((Some(x_min), Some(y_min)), (Some(x_max), Some(_))) = view_corners {
                let label_box_height = 14;
                let scale = Scale {
                    x: label_box_height as f32,
                    y: label_box_height as f32,
                };
                let font: Font<'static> = Font::try_from_bytes(font_data).unwrap();
                let white = [255, 255, 255];
                let alpha = 150;
                let f_inner_color = |rgb: &Rgb<u8>| util::apply_alpha(&rgb.0, &white, alpha);
                im = util::draw_bx_on_image(
                    im,
                    view_corners.0,
                    (Some(x_max), Some(y_min + label_box_height)),
                    &Rgb(white),
                    f_inner_color,
                );
                imageproc::drawing::draw_text_mut(
                    &mut im,
                    Rgb::<u8>([0, 0, 0]),
                    x_min as i32,
                    y_min as i32,
                    scale,
                    &font,
                    cats.label_of_box(box_idx),
                );
            }
        }
    }
    im
}

#[allow(clippy::needless_lifetimes)]
fn selected_or_deselected_indices<'a>(
    selected_bbs: &'a [bool],
    unselected: bool,
) -> impl Iterator<Item = usize> + Clone + 'a {
    selected_bbs
        .iter()
        .enumerate()
        .filter(move |(_, is_selected)| unselected ^ **is_selected)
        .map(|(i, _)| i)
}
#[allow(clippy::needless_lifetimes)]
fn deselected_indices<'a>(selected_bbs: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    selected_or_deselected_indices(selected_bbs, true)
}
#[allow(clippy::needless_lifetimes)]
fn selected_indices<'a>(selected_bbs: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    selected_or_deselected_indices(selected_bbs, false)
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BboxAnnotations {
    bbs: Vec<BB>,
    cat_ids: Vec<usize>,
    selected_bbs: Vec<bool>,
}
impl BboxAnnotations {
    pub const fn new() -> Self {
        BboxAnnotations {
            bbs: vec![],
            cat_ids: vec![],
            selected_bbs: vec![],
        }
    }
    pub fn from_bbs(bbs: Vec<BB>, cat_id: usize) -> BboxAnnotations {
        let bbs_len = bbs.len();
        BboxAnnotations {
            bbs,
            cat_ids: vec![cat_id; bbs_len],
            selected_bbs: vec![false; bbs_len],
        }
    }
    pub fn remove_cat(&mut self, cat_id: usize) {
        if cat_id > 0 {
            for cid in self.cat_ids.iter_mut() {
                if *cid >= cat_id {
                    *cid -= 1;
                }
            }
        }
    }
    pub fn remove(&mut self, box_idx: usize) -> BB {
        self.cat_ids.remove(box_idx);
        self.selected_bbs.remove(box_idx);
        self.bbs.remove(box_idx)
    }
    pub fn remove_selected(&mut self) {
        let keep_indices = deselected_indices(&self.selected_bbs);
        self.bbs = keep_indices
            .clone()
            .map(|i| self.bbs[i])
            .collect::<Vec<_>>();
        self.cat_ids = keep_indices.map(|i| self.cat_ids[i]).collect::<Vec<_>>();
        self.selected_bbs = vec![false; self.bbs.len()];
    }

    pub fn resize_bbs(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape) {
        let taken_bbs = mem::take(&mut self.bbs);
        self.bbs = resize_bbs(taken_bbs, &self.selected_bbs, x_shift, y_shift, shape_orig);
    }
    pub fn add_bb(&mut self, bb: BB, cat_id: usize) {
        self.cat_ids.push(cat_id);
        self.bbs.push(bb);
        self.selected_bbs.push(false);
    }
    pub fn bbs(&self) -> &Vec<BB> {
        &self.bbs
    }
    pub fn deselect(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = false;
    }
    pub fn toggle_selection(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = !self.selected_bbs[box_idx];
    }
    pub fn select(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = true;
    }
    pub fn selected_bbs(&self) -> &Vec<bool> {
        &self.selected_bbs
    }
    pub fn selected_follow_movement(
        &mut self,
        mpso: (u32, u32),
        mpo: (u32, u32),
        orig_shape: Shape,
    ) {
        for (bb, is_bb_selected) in self.bbs.iter_mut().zip(self.selected_bbs.iter()) {
            if *is_bb_selected {
                if let Some(bb_moved) = bb.follow_movement(mpso, mpo, orig_shape) {
                    *bb = bb_moved;
                }
            }
        }
    }
    pub fn label_selected(&mut self, cat_id: usize) {
        let selected_inds = selected_indices(&self.selected_bbs);
        for idx in selected_inds {
            self.cat_ids[idx] = cat_id;
        }
    }
    pub fn clear(&mut self) {
        self.bbs.clear();
        self.selected_bbs.clear();
        self.cat_ids.clear();
    }

    pub fn draw_on_view(
        &self,
        im_view: ViewImage,
        zoom_box: &Option<BB>,
        shape_orig: Shape,
        shape_win: Shape,
        labels: &[String],
        colors: &[[u8; 3]],
    ) -> ViewImage {
        draw_bbs(
            im_view,
            shape_orig,
            shape_win,
            zoom_box,
            &self.bbs,
            &self.selected_bbs,
            Cats {
                cat_ids: &self.cat_ids,
                colors,
                labels,
            },
        )
    }
}

#[cfg(test)]
fn make_test_bbs() -> Vec<BB> {
    vec![
        BB {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        },
        BB {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        },
        BB {
            x: 9,
            y: 9,
            w: 10,
            h: 10,
        },
    ]
}
#[test]
fn test_bbs() {
    let bbs = make_test_bbs();
    let shape_orig = Shape { w: 100, h: 100 };
    let resized = resize_bbs(bbs.clone(), &[false, true, true], -1, 1, shape_orig);
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BB::from_points((5, 5), (14, 16)), resized[1]);
    assert_eq!(BB::from_points((9, 9), (18, 20)), resized[2]);
}
#[test]
fn test_annos() {
    fn len_check(annos: &BboxAnnotations) {
        assert_eq!(annos.selected_bbs.len(), annos.bbs.len());
        assert_eq!(annos.cat_ids.len(), annos.bbs.len());
    }
    let mut annos = BboxAnnotations::from_bbs(make_test_bbs(), 0);
    len_check(&annos);
    let idx = 1;
    assert!(!annos.selected_bbs[idx]);
    annos.select(idx);
    len_check(&annos);
    annos.label_selected(3);
    len_check(&annos);
    for i in 0..(annos.bbs.len()) {
        if i == idx {
            assert_eq!(annos.cat_ids[i], 3);
        } else {
            assert_eq!(annos.cat_ids[i], 0);
        }
    }
    assert!(annos.selected_bbs[idx]);
    annos.deselect(idx);
    len_check(&annos);
    assert!(!annos.selected_bbs[idx]);
    annos.toggle_selection(idx);
    len_check(&annos);
    assert!(annos.selected_bbs[idx]);
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.cat_ids.len() == make_test_bbs().len() - 1);
    // this time nothing should be removed
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.cat_ids.len() == make_test_bbs().len() - 1);
    annos.remove(0);
    len_check(&annos);
    assert!(annos.bbs.len() == make_test_bbs().len() - 2);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 2);
    assert!(annos.cat_ids.len() == make_test_bbs().len() - 2);
    annos.add_bb(make_test_bbs()[0].clone(), 0);
    len_check(&annos);
    annos.add_bb(make_test_bbs()[0].clone(), 123);
    len_check(&annos);
    annos.clear();
    len_check(&annos);
    assert!(annos.bbs.len() == 0);
    assert!(annos.selected_bbs.len() == 0);
    assert!(annos.cat_ids.len() == 0);
    assert!(annos.cat_ids.len() == 0);
}
