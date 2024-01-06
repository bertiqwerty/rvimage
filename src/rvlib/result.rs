use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};
use tracing::error;
/// This will be thrown at you if the somehting within Exmex went wrong. Ok, obviously it is not an
/// exception, so thrown needs to be understood figuratively.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct RvError {
    msg: String,
}
impl RvError {
    pub fn new(msg: &str) -> RvError {
        RvError {
            msg: msg.to_string(),
        }
    }
    pub fn msg(&self) -> &str {
        &self.msg
    }
}
impl Display for RvError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}
impl Error for RvError {}

/// RV Image's result type with [`RvError`](RvError) as error type.
pub type RvResult<U> = Result<U, RvError>;

pub fn trace_ok<T, E>(x: Result<T, E>) -> Option<T>
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
/// Creates an [`RvError`](RvError) with a formatted message.
/// ```rust
/// # use std::error::Error;
/// use rvlib::{rverr, {result::RvError}};
/// # fn main() -> Result<(), Box<dyn Error>> {
/// assert_eq!(rverr!("some error {}", 1), RvError::new(format!("some error {}", 1).as_str()));
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! rverr {
    ($s:literal, $( $exps:expr ),*) => {
        $crate::result::RvError::new(format!($s, $($exps,)*).as_str())
    }
}

pub fn to_rv<E: Debug>(e: E) -> RvError {
    rverr!(
        "original error type is '{:?}', error message is '{:?}'",
        std::any::type_name::<E>(),
        e
    )
}
