use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum SortType {
    #[default]
    Natural,
    Alphabetical,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Eq, Debug, Clone, Copy)]
pub struct SortParams {
    pub kind: SortType,
    pub sort_by_filename: bool,
}
