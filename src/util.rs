use std::{ffi::OsStr, io};

pub fn osstr_to_str<'a>(p: Option<&'a OsStr>) -> io::Result<&'a str> {
    p.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{:?} not found", p)))?
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{:?} not convertible to unicode", p),
            )
        })
}
