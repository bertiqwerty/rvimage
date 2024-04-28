use std::fmt::Debug;
use tracing::{error, warn};
pub fn ignore_error<T, E>(x: Result<T, E>) -> Option<T>
where
    E: Debug,
{
    x.ok()
}
pub fn trace_ok_err<T, E>(x: Result<T, E>) -> Option<T>
where
    E: Debug,
{
    match x {
        Ok(x) => Some(x),
        Err(e) => {
            error!("{e:?}");
            None
        }
    }
}
pub fn trace_ok_warn<T, E>(x: Result<T, E>) -> Option<T>
where
    E: Debug,
{
    match x {
        Ok(x) => Some(x),
        Err(e) => {
            warn!("{e:?}");
            None
        }
    }
}
