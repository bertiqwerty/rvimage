"""Coco RLE helpers matching RV Image's Rust implementation.

Coco stores mask segmentations as run-length encodings (RLE) whose counts are
laid out in column-major (Fortran) order over the full image, with
size = [height, width]. Coco files exported by older RV Image versions used
row-major counts with size = [width, height] instead. The order of a given file
is detected from its info object, see detect_rle_order.

This module intentionally mirrors the logic in the Rust crates (rvimage-domain's
rle_*_to_* functions and coco_io.rs) so that masks round-trip identically
between the Rust app and Python.

Note: this is unrelated to rvimage.converters, whose mask_to_rle / rle_to_mask
operate on RV Image's native project format (bounding-box relative, row-major).
"""

from __future__ import annotations

from enum import Enum

import numpy as np

from rvimage._rle import mask_to_rle as _mask_to_rle, rle_to_mask as _rle_to_mask


# Key written into the Coco info object to mark the RLE order.
RVIMAGE_RLE_ORDER_KEY = "rvimage_rle_order"

# Substring of the info.description written by RV Image on export.
RVIMAGE_SIGNATURE = "created with RV Image"


class RleOrder(str, Enum):
    """Order in which the Coco RLE counts are laid out."""

    ROW_MAJOR = "row_major"
    COLUMN_MAJOR = "column_major"


def detect_rle_order(info: dict | None) -> RleOrder:
    """Determine the RLE order of a Coco file from its info object.

    Mirrors the detection in RV Image's convert_to_toolsdata:
    - an explicit rvimage_rle_order marker is trusted,
    - otherwise a file carrying the RV Image signature in its description is
      assumed to be a legacy (row-major) export,
    - anything else (e.g. external pycocotools files) follows the column-major
      Coco convention.
    """
    if info:
        order = info.get(RVIMAGE_RLE_ORDER_KEY)
        if order == RleOrder.ROW_MAJOR.value:
            return RleOrder.ROW_MAJOR
        if order is not None:
            return RleOrder.COLUMN_MAJOR
        description = info.get("description", "")
        if isinstance(description, str) and RVIMAGE_SIGNATURE in description:
            return RleOrder.ROW_MAJOR
    return RleOrder.COLUMN_MAJOR


def _size_to_wh(size: list[int], order: RleOrder) -> tuple[int, int]:
    if order == RleOrder.COLUMN_MAJOR:
        # Coco convention: size = [height, width]
        h, w = int(size[0]), int(size[1])
    else:
        # legacy RV Image: size = [width, height]
        w, h = int(size[0]), int(size[1])
    return w, h


def coco_rle_to_mask(
    counts: list[int],
    size: list[int],
    order: RleOrder = RleOrder.COLUMN_MAJOR,
    value: float = 1,
) -> np.ndarray:
    """Decode a Coco RLE into a full-image (height, width) mask.

    Args:
        counts: the RLE run lengths.
        size: the Coco size field ([height, width] for column-major,
            [width, height] for legacy row-major).
        order: the RLE order, see detect_rle_order.
        value: value written for foreground pixels.
    """
    w, h = _size_to_wh(size, order)
    # Decode directly in the file's order into a C-contiguous mask (no
    # transpose copy).
    return _rle_to_mask(
        counts, w, h, value, column_major=order == RleOrder.COLUMN_MAJOR
    )


def mask_to_coco_rle(
    mask: np.ndarray,
    order: RleOrder = RleOrder.COLUMN_MAJOR,
    intensity: float | None = None,
) -> dict:
    """Encode a full-image (height, width) mask into a Coco RLE dict.

    Returns a dict with counts and size; size follows the given order
    convention. When intensity is given it is added under the intensity key
    (an RV Image extension).
    """
    arr = np.asarray(mask)
    h, w = arr.shape[:2]
    counts = _mask_to_rle(arr, column_major=order == RleOrder.COLUMN_MAJOR)
    size = [int(h), int(w)] if order == RleOrder.COLUMN_MAJOR else [int(w), int(h)]
    seg: dict = {"counts": counts, "size": size}
    if intensity is not None:
        seg["intensity"] = intensity
    return seg


def segmentation_to_mask(
    segmentation: dict,
    order: RleOrder = RleOrder.COLUMN_MAJOR,
    value: float = 1,
) -> np.ndarray:
    """Decode an RLE segmentation dict ({counts, size}) into a mask."""
    return coco_rle_to_mask(segmentation["counts"], segmentation["size"], order, value)


def iter_rle_masks(coco: dict, value: float = 1):
    """Yield (annotation, mask) for every RLE-segmentation annotation.

    The RLE order is auto-detected once from coco["info"]. Annotations without
    an RLE segmentation (e.g. polygons or bare boxes) are skipped.
    """
    order = detect_rle_order(coco.get("info"))
    for ann in coco.get("annotations", []):
        seg = ann.get("segmentation")
        if isinstance(seg, dict) and "counts" in seg and "size" in seg:
            yield ann, segmentation_to_mask(seg, order, value)


def rle_order_info(order: RleOrder = RleOrder.COLUMN_MAJOR) -> dict:
    """Return an info fragment marking the RLE order (for exports)."""
    return {RVIMAGE_RLE_ORDER_KEY: order.value}
