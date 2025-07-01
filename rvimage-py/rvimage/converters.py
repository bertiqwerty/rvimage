from collections.abc import Sequence, Iterable
import numpy as np
import cv2

from rvimage.domain import BbF, BbI, Point


def rle_to_mask(rle: list[int], value: float, mask: np.ndarray | BbI) -> np.ndarray:
    if isinstance(mask, BbI):
        mask = np.zeros((mask.h, mask.w), dtype=np.uint8)
    else:
        mask = mask
    shape = mask.shape[:2]
    flat_mask = mask.ravel()
    pos = 0
    for i, n_elts in enumerate(rle):
        if i % 2 == 1:
            flat_mask[pos : pos + n_elts] = value
        pos = pos + n_elts
    return flat_mask.reshape(shape)


def mask_to_rle(mask: np.ndarray) -> list[int]:
    mask_w, mask_h = mask.shape
    flat_mask = mask.flatten()
    rle = []
    current_run = 0
    current_value = 0
    for y in range(mask_h):
        for x in range(mask_w):
            value = flat_mask[int(y * mask_w + x)]
            if value == current_value:
                current_run += 1
            else:
                rle.append(current_run)
                current_run = 1
                current_value = value
    rle.append(current_run)
    return rle


def fill_bbs_on_mask(
    bbs: Iterable[BbI | BbF],
    value: int,
    im_mask: np.ndarray,
    abs_coords_input: bool = True,
):
    if abs_coords_input:
        h, w = 1, 1
    else:
        h, w = im_mask.shape

    im_mask = im_mask.copy()
    for bb in bbs:
        if isinstance(bb, BbF):
            bb = bb.scale(w, h).to_bbi()
        im_mask[bb.slices] = value


def fill_polys_on_mask(
    polygons: Iterable[Sequence[Point]],
    value: int,
    im_mask: np.ndarray,
    abs_coords_input: bool = True,
):
    if abs_coords_input:
        h, w = 1, 1
    else:
        h, w = im_mask.shape

    im_mask = im_mask.copy()
    for poly in polygons:
        polygon_ = np.round(np.array([[[p.x * w, p.y * h] for p in poly]])).astype(
            np.int32
        )
        im_mask = cv2.fillPoly(img=im_mask, pts=polygon_, color=value)  # type: ignore
    return im_mask


def extract_polys_from_mask(
    im_mask: np.ndarray, abs_coords_output: bool
) -> list[list[Point]]:
    contours, _ = cv2.findContours(im_mask, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
    polygons = []
    h, w = im_mask.shape

    for obj in contours:
        polygon = []

        for point in obj:
            assert isinstance(point, np.ndarray)
            if abs_coords_output:
                x, y = point[0][0], point[0][1]
            else:
                x, y = point[0][0] / w, point[0][1] / h
            polygon.append(Point(x=x, y=y))

        polygons.append(polygon)
    return polygons


def decode_bytes_into_rgbarray(
    bytes: bytes, color_mode: int = cv2.IMREAD_COLOR
) -> np.ndarray:
    np_bytes = np.frombuffer(bytes, np.uint8)
    im = cv2.imdecode(np_bytes, color_mode)
    im = cv2.cvtColor(im, cv2.COLOR_BGR2RGB)
    if im is None:
        raise ValueError("Could not decode image from uploaded bytes")
    return im
