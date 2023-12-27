use crate::{
    domain::{Annotate, BB},
    result::RvResult,
    rverr,
    util::true_indices,
    Shape,
};
use serde::{Deserialize, Serialize};
use std::mem;
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct InstanceAnnotations<T> {
    elts: Vec<T>,
    cat_idxs: Vec<usize>,
    selected_mask: Vec<bool>,
}

impl<T> Eq for InstanceAnnotations<T> where T: PartialEq + Eq {}

impl<T> InstanceAnnotations<T>
where
    T: Annotate + PartialEq + Default,
{
    pub fn new(elts: Vec<T>, cat_idxs: Vec<usize>, selected_mask: Vec<bool>) -> RvResult<Self> {
        if elts.len() != cat_idxs.len() || elts.len() != selected_mask.len() {
            Err(rverr!(
                "All inputs need same length. got {}, {}, {} for elts, cat_idxs, selected_mask",
                elts.len(),
                cat_idxs.len(),
                selected_mask.len()
            ))
        } else {
            Ok(Self {
                elts,
                cat_idxs,
                selected_mask,
            })
        }
    }
    pub fn separate_data(self) -> (Vec<T>, Vec<usize>, Vec<bool>) {
        (self.elts, self.cat_idxs, self.selected_mask)
    }
    pub fn extend<IE, IC>(&mut self, elts: IE, cat_ids: IC, shape_image: Shape)
    where
        IE: Iterator<Item = T>,
        IC: Iterator<Item = usize>,
    {
        for (elt, cat_id) in elts.zip(cat_ids) {
            if elt.is_contained_in_image(shape_image) && !self.elts.contains(&elt) {
                self.add_elt(elt, cat_id)
            }
        }
    }
    pub fn add_elt(&mut self, elt: T, cat_idx: usize) {
        self.cat_idxs.push(cat_idx);
        self.elts.push(elt);
        self.selected_mask.push(false);
    }
    pub fn cat_idxs(&self) -> &Vec<usize> {
        &self.cat_idxs
    }

    pub fn elts_iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.elts.iter_mut()
    }

    pub fn elts(&self) -> &Vec<T> {
        &self.elts
    }
    pub fn deselect(&mut self, box_idx: usize) {
        self.selected_mask[box_idx] = false;
    }

    pub fn deselect_all(&mut self) {
        for s in &mut self.selected_mask {
            *s = false;
        }
    }

    pub fn toggle_selection(&mut self, elt_idx: usize) {
        let is_selected = self.selected_mask[elt_idx];
        if is_selected {
            self.deselect(elt_idx);
        } else {
            self.select(elt_idx);
        }
    }

    pub fn select(&mut self, box_idx: usize) {
        self.selected_mask[box_idx] = true;
    }

    pub fn select_multi(&mut self, box_idxs: impl Iterator<Item = usize>) {
        for box_idx in box_idxs {
            self.select(box_idx);
        }
    }

    pub fn select_all(&mut self) {
        let n_bbs = self.elts.len();
        self.select_multi(0..n_bbs);
    }

    pub fn select_last_n(&mut self, n: usize) {
        let len = self.elts.len();
        self.select_multi((len - n)..len);
    }

    pub fn selected_mask(&self) -> &Vec<bool> {
        &self.selected_mask
    }
    pub fn from_elts_cats(elts: Vec<T>, cat_ids: Vec<usize>) -> Self {
        let n_elts = elts.len();
        Self {
            elts,
            cat_idxs: cat_ids,
            selected_mask: vec![false; n_elts],
        }
    }

    pub fn label_selected(&mut self, cat_id: usize) {
        let selected_inds = true_indices(&self.selected_mask);
        for idx in selected_inds {
            self.cat_idxs[idx] = cat_id;
        }
    }

    pub fn clear(&mut self) {
        self.elts.clear();
        self.selected_mask.clear();
        self.cat_idxs.clear();
    }
    pub fn reduce_cat_idxs(&mut self, cat_idx: usize) {
        for cid in self.cat_idxs.iter_mut() {
            if *cid >= cat_idx && *cid > 0 {
                *cid -= 1;
            }
        }
    }

    pub fn remove(&mut self, box_idx: usize) -> T {
        self.cat_idxs.remove(box_idx);
        self.selected_mask.remove(box_idx);
        self.elts.remove(box_idx)
    }

    pub fn remove_multiple(&mut self, indices: &[usize]) {
        let keep_indices = (0..self.elts.len()).filter(|i| !indices.contains(i));
        self.elts = keep_indices
            .clone()
            .map(|i| mem::take(&mut self.elts[i]))
            .collect::<Vec<_>>();
        self.cat_idxs = keep_indices.map(|i| self.cat_idxs[i]).collect::<Vec<_>>();
        self.selected_mask = vec![false; self.elts.len()];
    }

    pub fn remove_selected(&mut self) {
        let selected = true_indices(self.selected_mask());
        self.remove_multiple(&selected.collect::<Vec<_>>());
    }
}

pub fn resize_bbs_inds<F>(
    mut bbs: Vec<BB>,
    bb_inds: impl Iterator<Item = usize>,
    resize: F,
) -> Vec<BB>
where
    F: Fn(BB) -> Option<BB>,
{
    for idx in bb_inds {
        if let Some(bb) = resize(bbs[idx]) {
            bbs[idx] = bb;
        }
    }
    bbs
}
pub fn resize_bbs<F>(bbs: Vec<BB>, selected_bbs: &[bool], resize: F) -> Vec<BB>
where
    F: Fn(BB) -> Option<BB>,
{
    let selected_idxs = true_indices(selected_bbs);
    resize_bbs_inds(bbs, selected_idxs, resize)
}
