use crate::{
    domain::{Shape, BB},
    image_util,
    types::ViewImage,
};
use image::Rgb;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use std::mem;
const BBOX_ALPHA: u8 = 180;
const BBOX_ALPHA_SELECTED: u8 = 120;

fn resize_bbs<F>(mut bbs: Vec<BB>, selected_bbs: &[bool], resize: F) -> Vec<BB>
where
    F: Fn(BB) -> Option<BB>,
{
    let selected_idxs = selected_bbs
        .iter()
        .enumerate()
        .filter(|(_, x)| **x)
        .map(|(i, _)| i);
    for idx in selected_idxs {
        if let Some(bb) = resize(bbs[idx]) {
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
    show_label: bool,
) -> ViewImage {
    let font_data: &[u8] = include_bytes!("../../../../resources/Roboto/Roboto-Bold.ttf");
    // remove those box ids that are outside of the zoom box
    let relevant_box_inds = (0..bbs.len()).filter(|box_idx| {
        if let Some(zb) = zoom_box {
            !(bbs[*box_idx].y + bbs[*box_idx].h < zb.y
                || bbs[*box_idx].x + bbs[*box_idx].w < zb.x
                || bbs[*box_idx].x > zb.x + zb.w
                || bbs[*box_idx].y > zb.y + zb.h)
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
            |rgb: &Rgb<u8>| image_util::apply_alpha(&rgb.0, cats.color_of_box(box_idx), alpha);
        let view_corners = bbs[box_idx].to_view_corners(shape_orig, shape_win, zoom_box);

        let color_rgb = Rgb(*cats.color_of_box(box_idx));
        im = image_util::draw_bx_on_image(
            im,
            view_corners.0,
            view_corners.1,
            &color_rgb,
            f_inner_color,
        );

        // draw label field
        // we do not the label field for the empty-string-label
        if !cats.label_of_box(box_idx).is_empty() && show_label {
            if let ((Some(x_min), Some(y_min)), (Some(x_max), Some(_))) = view_corners {
                let label_box_height = 14;
                let scale = Scale {
                    x: label_box_height as f32,
                    y: label_box_height as f32,
                };
                let font: Font<'static> = Font::try_from_bytes(font_data).unwrap();
                let white = [255, 255, 255];
                let alpha = 150;
                let f_inner_color = |rgb: &Rgb<u8>| image_util::apply_alpha(&rgb.0, &white, alpha);
                im = image_util::draw_bx_on_image(
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
    let res = selected_bbs
        .iter()
        .enumerate()
        .filter(move |(_, is_selected)| unselected ^ **is_selected)
        .map(|(i, _)| i);
    res
}

#[allow(clippy::needless_lifetimes)]
fn deselected_indices<'a>(selected_bbs: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    selected_or_deselected_indices(selected_bbs, true)
}

#[allow(clippy::needless_lifetimes)]
pub fn selected_indices<'a>(selected_bbs: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    selected_or_deselected_indices(selected_bbs, false)
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BboxAnnotations {
    bbs: Vec<BB>,
    cat_idxs: Vec<usize>,
    selected_bbs: Vec<bool>,
    pub show_labels: bool,
}

impl BboxAnnotations {
    pub const fn new() -> Self {
        BboxAnnotations {
            bbs: vec![],
            cat_idxs: vec![],
            selected_bbs: vec![],
            show_labels: false,
        }
    }

    pub fn to_data(self) -> (Vec<BB>, Vec<usize>) {
        (self.bbs, self.cat_idxs)
    }

    pub fn extend<IB, IC>(&mut self, bbs: IB, cat_ids: IC, shape_image: Shape)
    where
        IB: Iterator<Item = BB>,
        IC: Iterator<Item = usize>,
    {
        for (bb, cat_id) in bbs.zip(cat_ids) {
            if bb.is_contained_in(shape_image) && !self.bbs().contains(&bb) {
                self.add_bb(bb, cat_id)
            }
        }
    }

    pub fn from_bbs_cats(bbs: Vec<BB>, cat_ids: Vec<usize>) -> BboxAnnotations {
        let bbs_len = bbs.len();
        BboxAnnotations {
            bbs,
            cat_idxs: cat_ids,
            selected_bbs: vec![false; bbs_len],
            show_labels: false,
        }
    }

    pub fn from_bbs(bbs: Vec<BB>, cat_id: usize) -> BboxAnnotations {
        let bbs_len = bbs.len();
        BboxAnnotations {
            bbs,
            cat_idxs: vec![cat_id; bbs_len],
            selected_bbs: vec![false; bbs_len],
            show_labels: false,
        }
    }

    pub fn reduce_cat_idxs(&mut self, cat_idx: usize) {
        if cat_idx > 0 {
            for cid in self.cat_idxs.iter_mut() {
                if *cid >= cat_idx {
                    *cid -= 1;
                }
            }
        }
    }

    pub fn remove(&mut self, box_idx: usize) -> BB {
        self.cat_idxs.remove(box_idx);
        self.selected_bbs.remove(box_idx);
        self.bbs.remove(box_idx)
    }

    pub fn remove_selected(&mut self) {
        let keep_indices = deselected_indices(&self.selected_bbs);
        self.bbs = keep_indices
            .clone()
            .map(|i| self.bbs[i])
            .collect::<Vec<_>>();
        self.cat_idxs = keep_indices.map(|i| self.cat_idxs[i]).collect::<Vec<_>>();
        self.selected_bbs = vec![false; self.bbs.len()];
    }

    pub fn shift(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape) {
        let taken_bbs = mem::take(&mut self.bbs);
        self.bbs = resize_bbs(taken_bbs, &self.selected_bbs, |bb| {
            bb.translate(x_shift, y_shift, shape_orig)
        });
    }
    pub fn shift_min_bbs(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape) {
        let taken_bbs = mem::take(&mut self.bbs);
        self.bbs = resize_bbs(taken_bbs, &self.selected_bbs, |bb| {
            bb.shift_min(x_shift, y_shift, shape_orig)
        });
    }

    pub fn shift_max_bbs(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape) {
        let taken_bbs = mem::take(&mut self.bbs);
        self.bbs = resize_bbs(taken_bbs, &self.selected_bbs, |bb| {
            bb.shift_max(x_shift, y_shift, shape_orig)
        });
    }

    pub fn add_bb(&mut self, bb: BB, cat_idx: usize) {
        self.cat_idxs.push(cat_idx);
        self.bbs.push(bb);
        self.selected_bbs.push(false);
    }

    pub fn cat_idxs(&self) -> &Vec<usize> {
        &self.cat_idxs
    }

    pub fn bbs(&self) -> &Vec<BB> {
        &self.bbs
    }

    pub fn deselect(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = false;
    }

    pub fn select_all(&mut self) {
        for s in &mut self.selected_bbs {
            *s = true;
        }
    }

    pub fn deselect_all(&mut self) {
        for s in &mut self.selected_bbs {
            *s = false;
        }
    }

    pub fn toggle_selection(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = !self.selected_bbs[box_idx];
    }

    pub fn select(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = true;
    }

    pub fn select_last(&mut self) {
        self.selected_bbs[self.bbs.len() - 1] = true;
    }

    pub fn selected_bbs(&self) -> &Vec<bool> {
        &self.selected_bbs
    }

    pub fn selected_follow_movement(
        &mut self,
        mpso: (u32, u32),
        mpo: (u32, u32),
        orig_shape: Shape,
    ) -> bool {
        let mut move_somebody = false;
        for (bb, is_bb_selected) in self.bbs.iter_mut().zip(self.selected_bbs.iter()) {
            if *is_bb_selected {
                if let Some(bb_moved) = bb.follow_movement(mpso, mpo, orig_shape) {
                    move_somebody = true;
                    *bb = bb_moved;
                }
            }
        }
        move_somebody
    }

    pub fn label_selected(&mut self, cat_id: usize) {
        let selected_inds = selected_indices(&self.selected_bbs);
        for idx in selected_inds {
            self.cat_idxs[idx] = cat_id;
        }
    }

    pub fn clear(&mut self) {
        self.bbs.clear();
        self.selected_bbs.clear();
        self.cat_idxs.clear();
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
                cat_ids: &self.cat_idxs,
                colors,
                labels,
            },
            self.show_labels,
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

    // shift max
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_max(-1, 1, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BB::from_points((5, 5), (14, 16)), resized[1]);
    assert_eq!(BB::from_points((9, 9), (18, 20)), resized[2]);

    // shift min
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_min(-1, 1, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BB::from_points((4, 6), (15, 15)), resized[1]);
    assert_eq!(BB::from_points((8, 10), (19, 19)), resized[2]);
}
#[test]
fn test_annos() {
    fn len_check(annos: &BboxAnnotations) {
        assert_eq!(annos.selected_bbs.len(), annos.bbs.len());
        assert_eq!(annos.cat_idxs.len(), annos.bbs.len());
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
            assert_eq!(annos.cat_idxs[i], 3);
        } else {
            assert_eq!(annos.cat_idxs[i], 0);
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
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 1);
    // this time nothing should be removed
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 1);
    annos.remove(0);
    len_check(&annos);
    assert!(annos.bbs.len() == make_test_bbs().len() - 2);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 2);
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 2);
    annos.add_bb(make_test_bbs()[0].clone(), 0);
    len_check(&annos);
    annos.add_bb(make_test_bbs()[0].clone(), 123);
    len_check(&annos);
    annos.clear();
    len_check(&annos);
    assert!(annos.bbs.len() == 0);
    assert!(annos.selected_bbs.len() == 0);
    assert!(annos.cat_idxs.len() == 0);
    assert!(annos.cat_idxs.len() == 0);
}
