use crate::{
    domain::{BbPointIterator, MakeDrawable, OutOfBoundsMode, Shape, BB},
    image_util,
    types::ViewImage,
    util::true_indices,
};
use image::Rgb;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use std::mem;

use super::bbox_splitmode::SplitMode;
const BBOX_ALPHA: u8 = 180;
const BBOX_ALPHA_SELECTED: u8 = 120;

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

struct BbParams<'a> {
    pub bbs: &'a [InstanceGeo],
    pub selected_bbs: &'a [bool],
    pub cats: Cats<'a>,
    show_label: bool,
}

fn draw_bbs(
    mut im_view: ViewImage,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    bb_params: BbParams,
) -> ViewImage {
    let font_data: &[u8] = include_bytes!("../../../../resources/Roboto/Roboto-Bold.ttf");
    let BbParams {
        bbs,
        selected_bbs,
        cats,
        show_label,
    } = bb_params;
    // remove those box ids that are outside of the zoom box
    let relevant_box_inds = (0..bbs.len()).filter(|box_idx| {
        if let Some(zb) = zoom_box {
            let encl_b = bbs[*box_idx].enclosing_bb();
            !(encl_b.y + encl_b.h < zb.y
                || encl_b.x + encl_b.w < zb.x
                || encl_b.x > zb.x + zb.w
                || encl_b.y > zb.y + zb.h)
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

        let (boundary_points, inner_points) =
            bbs[box_idx].points_on_view(Shape::from_im(&im_view), shape_orig, shape_win, zoom_box);
        let color_rgb = Rgb(*cats.color_of_box(box_idx));
        im_view = image_util::draw_on_image(
            im_view,
            boundary_points,
            inner_points,
            &color_rgb,
            f_inner_color,
        );

        // draw label field, we do show not the label field for empty-string-labels
        if !cats.label_of_box(box_idx).is_empty() && show_label {
            let encl_viewcorners = bbs[box_idx]
                .enclosing_bb()
                .to_viewcorners(shape_orig, shape_win, zoom_box);
            if let Some((x_min, y_min, x_max, _)) = encl_viewcorners.to_optional_tuple() {
                let label_box_height = 14;
                let scale = Scale {
                    x: label_box_height as f32,
                    y: label_box_height as f32,
                };
                let font: Font<'static> = Font::try_from_bytes(font_data).unwrap();
                let white = [255, 255, 255];
                let alpha = 150;
                let f_inner_color = |rgb: &Rgb<u8>| image_util::apply_alpha(&rgb.0, &white, alpha);
                let label_view_box =
                    BB::from_points((x_min, y_min), (x_max, y_min + label_box_height));
                let inner_points = BbPointIterator::from_bb(label_view_box);
                let boundary_points = label_view_box.corners();
                im_view = image_util::draw_on_image(
                    im_view,
                    boundary_points,
                    inner_points,
                    &Rgb(white),
                    f_inner_color,
                );
                imageproc::drawing::draw_text_mut(
                    &mut im_view,
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
    im_view
}

pub type InstanceGeo = BB;

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BboxAnnotations {
    bbs: Vec<InstanceGeo>,
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

    pub fn to_data(self) -> (Vec<InstanceGeo>, Vec<usize>) {
        (self.bbs, self.cat_idxs)
    }

    pub fn extend<IB, IC>(&mut self, bbs: IB, cat_ids: IC, shape_image: Shape)
    where
        IB: Iterator<Item = InstanceGeo>,
        IC: Iterator<Item = usize>,
    {
        for (bb, cat_id) in bbs.zip(cat_ids) {
            if bb.is_contained_in_image(shape_image) && !self.bbs().contains(&bb) {
                self.add_bb(bb, cat_id)
            }
        }
    }

    pub fn from_bbs_cats(bbs: Vec<InstanceGeo>, cat_ids: Vec<usize>) -> BboxAnnotations {
        let bbs_len = bbs.len();
        BboxAnnotations {
            bbs,
            cat_idxs: cat_ids,
            selected_bbs: vec![false; bbs_len],
            show_labels: false,
        }
    }

    pub fn from_bbs(bbs: Vec<InstanceGeo>, cat_id: usize) -> BboxAnnotations {
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

    pub fn remove(&mut self, box_idx: usize) -> InstanceGeo {
        self.cat_idxs.remove(box_idx);
        self.selected_bbs.remove(box_idx);
        self.bbs.remove(box_idx)
    }

    pub fn remove_multiple(&mut self, indices: &[usize]) {
        let keep_indices = (0..self.bbs.len()).filter(|i| !indices.contains(i));
        self.bbs = keep_indices
            .clone()
            .map(|i| self.bbs[i])
            .collect::<Vec<_>>();
        self.cat_idxs = keep_indices.map(|i| self.cat_idxs[i]).collect::<Vec<_>>();
        self.selected_bbs = vec![false; self.bbs.len()];
    }

    pub fn remove_selected(&mut self) {
        let selected = true_indices(self.selected_bbs());
        self.remove_multiple(&selected.collect::<Vec<_>>());
    }

    pub fn shift(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape, split_mode: SplitMode) {
        self.shift_min_bbs(x_shift, y_shift, shape_orig, split_mode);
        self.shift_max_bbs(x_shift, y_shift, shape_orig, split_mode);
    }
    pub fn shift_min_bbs(
        &mut self,
        x_shift: i32,
        y_shift: i32,
        shape_orig: Shape,
        split_mode: SplitMode,
    ) {
        self.bbs = split_mode.shift_min_bbs(
            x_shift,
            y_shift,
            &self.selected_bbs,
            mem::take(&mut self.bbs),
            shape_orig,
        );
    }

    pub fn shift_max_bbs(
        &mut self,
        x_shift: i32,
        y_shift: i32,
        shape_orig: Shape,
        split_mode: SplitMode,
    ) {
        self.bbs = split_mode.shift_max_bbs(
            x_shift,
            y_shift,
            &self.selected_bbs,
            mem::take(&mut self.bbs),
            shape_orig,
        );
    }

    pub fn add_bb(&mut self, bb: InstanceGeo, cat_idx: usize) {
        self.cat_idxs.push(cat_idx);
        self.bbs.push(bb);
        self.selected_bbs.push(false);
    }

    pub fn cat_idxs(&self) -> &Vec<usize> {
        &self.cat_idxs
    }

    pub fn bbs(&self) -> &Vec<InstanceGeo> {
        &self.bbs
    }

    pub fn deselect(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = false;
    }

    pub fn deselect_all(&mut self) {
        for s in &mut self.selected_bbs {
            *s = false;
        }
    }

    pub fn toggle_selection(&mut self, box_idx: usize) {
        let is_selected = self.selected_bbs[box_idx];
        if is_selected {
            self.deselect(box_idx);
        } else {
            self.select(box_idx);
        }
    }

    pub fn select(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = true;
    }

    pub fn select_multi(&mut self, box_idxs: impl Iterator<Item = usize>) {
        for box_idx in box_idxs {
            self.select(box_idx);
        }
    }

    pub fn select_all(&mut self) {
        let n_bbs = self.bbs.len();
        self.select_multi(0..n_bbs);
    }

    pub fn select_last_n(&mut self, n: usize) {
        let len = self.bbs.len();
        self.select_multi((len - n)..len);
    }

    pub fn selected_bbs(&self) -> &Vec<bool> {
        &self.selected_bbs
    }

    pub fn selected_follow_movement(
        &mut self,
        mpo_from: (u32, u32),
        mpo_to: (u32, u32),
        orig_shape: Shape,
        oob_mode: OutOfBoundsMode,
        split_mode: SplitMode,
    ) -> bool {
        let mut moved_somebody = false;
        for (bb, is_bb_selected) in self.bbs.iter_mut().zip(self.selected_bbs.iter()) {
            if *is_bb_selected {
                (moved_somebody, *bb) =
                    split_mode.bb_follow_movement(*bb, mpo_from, mpo_to, orig_shape, oob_mode)
            }
        }
        moved_somebody
    }

    pub fn label_selected(&mut self, cat_id: usize) {
        let selected_inds = true_indices(&self.selected_bbs);
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
            BbParams {
                bbs: &self.bbs,
                selected_bbs: &self.selected_bbs,
                cats: Cats {
                    cat_ids: &self.cat_idxs,
                    colors,
                    labels,
                },
                show_label: self.show_labels,
            },
        )
    }
}
#[cfg(test)]
use super::core::resize_bbs;
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
    assert_eq!(InstanceGeo::from_points((5, 5), (14, 16)), resized[1]);
    assert_eq!(InstanceGeo::from_points((9, 9), (18, 20)), resized[2]);

    // shift min
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_min(-1, 1, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(InstanceGeo::from_points((4, 6), (15, 15)), resized[1]);
    assert_eq!(InstanceGeo::from_points((8, 10), (19, 19)), resized[2]);
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
