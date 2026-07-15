"""Shared low-level run-length encoding core.

Encoding operates on a flat 1-D array and is agnostic to order. Decoding is
order-aware and always produces a C-contiguous (height, width) mask:
- row-major RLE is filled as contiguous slices,
- column-major (Coco/Fortran) RLE is filled as strided column segments.

Filling in the requested order directly avoids materialising a transposed
(F-contiguous) intermediate and copying it, so the returned mask is cache
friendly for the row-major consumers used elsewhere without an extra copy.
"""

from __future__ import annotations

import numpy as np


def flat_to_rle(flat: np.ndarray) -> list[int]:
    """Encode a flat binary array into run lengths.

    The result follows the Coco/RV Image convention where counts[0] is a
    background run (possibly zero-length), then alternating foreground and
    background runs.
    """
    binary = (np.asarray(flat) != 0).astype(np.int8)
    if binary.size == 0:
        return [0]
    change = np.flatnonzero(np.diff(binary)) + 1
    bounds = np.concatenate(([0], change, [binary.size]))
    runs = np.diff(bounds).astype(int).tolist()
    # Coco counts start with a background run.
    if binary[0] == 1:
        return [0] + runs
    return runs


def mask_to_rle(mask: np.ndarray, column_major: bool = False) -> list[int]:
    """Encode a 2-D (height, width) mask into run lengths.

    column_major selects Fortran-order (Coco) traversal; otherwise C-order
    (row-major) is used.
    """
    arr = np.asarray(mask)
    return flat_to_rle(arr.flatten(order="F" if column_major else "C"))


def rle_to_mask(
    counts: list[int],
    w: int,
    h: int,
    value: float = 1,
    column_major: bool = False,
) -> np.ndarray:
    """Decode run lengths into a C-contiguous (h, w) mask.

    Odd-indexed runs are foreground and filled with value. The fill loop is
    chosen from column_major so the mask is built row-major-contiguous
    directly, without a transpose copy.
    """
    dtype = np.uint8 if isinstance(value, (int, np.integer)) else np.float64
    mask = np.zeros((h, w), dtype=dtype)
    flat = mask.reshape(-1)  # C-contiguous view, flat index == y * w + x
    total = h * w
    pos = 0
    if not column_major:
        for i, n_elts in enumerate(counts):
            n_elts = int(n_elts)
            if i % 2 == 1 and pos < total:
                flat[pos : min(pos + n_elts, total)] = value
            pos += n_elts
    else:
        for i, n_elts in enumerate(counts):
            n_elts = int(n_elts)
            if i % 2 == 1:
                k = pos
                end = min(pos + n_elts, total)
                while k < end:
                    # column-major linear index k -> column x, row y
                    x, y = divmod(k, h)
                    seg = min(end - k, h - y)  # stay within column x
                    start = y * w + x
                    flat[start : start + seg * w : w] = value
                    k += seg
            pos += n_elts
    return mask
