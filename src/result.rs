use std::{
    error::Error,
    fmt::{self, Display, Formatter, Debug},
};

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
}
impl Display for RvError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}
impl Error for RvError {}

/// Rimview's result type with [`RvError`](RvError) as error type.
pub type RvResult<U> = Result<U, RvError>;

/// Creates an [`RvError`](RvError) with a formatted message.
/// ```rust
/// # use std::error::Error;
/// use crate::result::{format_exerr, RvError};
/// # fn main() -> Result<(), Box<dyn Error>> {
/// assert_eq!(format_rverr!("some error {}", 1), RvError::new(format!("some error {}", 1).as_str()));
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! format_rverr {
    ($s:literal, $( $exps:expr ),*) => {
        RvError::new(format!($s, $($exps,)*).as_str())
    }
}

pub fn to_rv<E: Debug>(e: E) -> RvError {
    format_rverr!("{:?}", e)
}