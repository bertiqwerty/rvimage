use numpy::{PyReadonlyArray1, PyReadwriteArray2};
use pyo3::prelude::*;
use rvimage_domain;

#[pyfunction]
fn rle_to_mask_inplace(
    rle: PyReadonlyArray1<u32>,
    mut mask: PyReadwriteArray2<u8>,
    w: usize,
) -> PyResult<()> {
    let rle = rle.as_slice();
    let mask = mask.as_slice_mut();
    if let (Ok(mask), Ok(rle)) = (mask, rle) {
        Ok(rvimage_domain::rle_to_mask_inplace(rle, mask, w as u32))
    } else {
        Err(pyo3::exceptions::PyValueError::new_err(
            "Failed to convert RLE to slice",
        ))
    }
}
#[pymodule]
fn rvimage(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rle_to_mask_inplace, m)?)?;
    Ok(())
}
