mod server;
use rvimage_domain::{RvResult, to_rv};
use serde::Serialize;
pub use server::{CmdServer, WandServer};

pub fn serialize_or_default<T: Default + Serialize>(x: Option<&T>) -> RvResult<String> {
    {
        if let Some(x) = x {
            serde_json::to_string(x)
        } else {
            serde_json::to_string(&T::default())
        }
    }
    .map_err(to_rv)
}
