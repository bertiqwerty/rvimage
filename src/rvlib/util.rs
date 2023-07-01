#[allow(clippy::needless_lifetimes)]
fn true_or_false<'a>(
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
pub fn true_indices<'a>(selected_bbs: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    true_or_false(selected_bbs, false)
}
