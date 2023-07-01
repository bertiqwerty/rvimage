use crate::{domain::BB, util::true_indices};

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
