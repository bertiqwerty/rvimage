use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};
/// This will be thrown at you if the somehting within Exmex went wrong. Ok, obviously it is not an
/// exception, so thrown needs to be understood figuratively.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct RvError {
    msg: String,
}
impl RvError {
    #[must_use]
    pub fn new(msg: &str) -> RvError {
        RvError {
            msg: msg.to_string(),
        }
    }
    #[must_use]
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
impl From<&str> for RvError {
    fn from(value: &str) -> Self {
        RvError::new(value)
    }
}
/// RV Image's result type with [`RvError`](RvError) as error type.
pub type RvResult<U> = Result<U, RvError>;

/// Creates an [`RvError`](RvError) with a formatted message.
/// ```rust
/// # use std::error::Error;
/// use rvimage_domain::{rverr, {result::RvError}};
/// # fn main() -> Result<(), Box<dyn Error>> {
/// assert_eq!(rverr!("some error {}", 1), RvError::new(format!("some error {}", 1).as_str()));
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! rverr {
    ($s:literal) => {
        $crate::result::RvError::new(format!($s).as_str())
    };
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
