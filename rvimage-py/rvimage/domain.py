from __future__ import annotations

import math
from typing import Generic, TypeVar

import numpy as np
from pydantic import BaseModel
from scipy.ndimage import find_objects
from scipy.ndimage import label as scpiy_label

T = TypeVar("T", int, float)


class _RowColMixin(Generic[T]):
    @property
    def r_min(self) -> T:
        return self.y  # type: ignore[attr-defined]

    @property
    def r_max(self) -> T:
        return self.y + self.h  # type: ignore[attr-defined]

    @property
    def c_min(self) -> T:
        return self.x  # type: ignore[attr-defined]

    @property
    def c_max(self) -> T:
        return self.x + self.w  # type: ignore[attr-defined]

    @property
    def width(self) -> T:
        return self.w  # type: ignore[attr-defined]

    @property
    def height(self) -> T:
        return self.h  # type: ignore[attr-defined]

    @classmethod
    def from_rowcols(cls, r_min: T, c_min: T, r_max: T, c_max: T):
        x = c_min
        y = r_min
        w = c_max - c_min
        h = r_max - r_min
        return cls(x=x, y=y, h=h, w=w)  # type: ignore[attr-defined]


class _ContainsMixin:
    def __contains__(self, other: BbI | BbF) -> bool:
        return (
            other.c_min >= self.c_min  # type: ignore[attr-defined]
            and other.c_max <= self.c_max  # type: ignore[attr-defined]
            and other.r_min >= self.r_min  # type: ignore[attr-defined]
            and other.r_max <= self.r_max  # type: ignore[attr-defined]
        )

    def intersect(self, other: BbI | BbF) -> BbI | BbF | None:
        c_min = max(self.c_min, other.c_min)  # type: ignore[attr-defined]
        c_max = min(self.c_max, other.c_max)  # type: ignore[attr-defined]
        r_min = max(self.r_min, other.r_min)  # type: ignore[attr-defined]
        r_max = min(self.r_max, other.r_max)  # type: ignore[attr-defined]
        if c_min < c_max and r_min < r_max:
            if (
                isinstance(c_min, int)
                and isinstance(c_max, int)
                and isinstance(r_min, int)
                and isinstance(r_max, int)
            ):
                return BbI.from_rowcols(r_min, c_min, r_max, c_max)
            else:
                return BbF.from_rowcols(r_min, c_min, r_max, c_max)
        else:
            # boxes don't overlap
            return None


class BbI(BaseModel, _RowColMixin[int], _ContainsMixin):
    x: int
    y: int
    w: int
    h: int

    @classmethod
    def from_slices(cls, slices: tuple[slice, slice]):
        x = slices[1].start
        y = slices[0].start
        w = slices[1].stop - x
        h = slices[0].stop - y
        return cls(x=x, y=y, w=w, h=h)

    @property
    def slices(self) -> tuple[slice, slice]:
        """
        Returns the slices for indexing a numpy array.
        """
        return slice(self.y, self.y + self.h), slice(self.x, self.x + self.w)


class BbF(BaseModel, _RowColMixin[float], _ContainsMixin):
    x: float
    y: float
    w: float
    h: float

    def to_bbi(self) -> BbI:
        """
        Convert to integer bounding box.
        """
        return BbI(
            x=int(np.round(self.x)),
            y=int(np.round(self.y)),
            w=int(np.round(self.w)),
            h=int(np.round(self.h)),
        )

    @classmethod
    def from_bbi(cls, bbi: BbI):
        return cls(x=bbi.x, y=bbi.y, w=bbi.w, h=bbi.h)

    def scale(self, scale_x: float, scale_y) -> "BbF":
        """
        Scale the bounding box by a factor.
        """
        return BbF(
            x=self.x * scale_x,
            y=self.y * scale_y,
            w=self.w * scale_x,
            h=self.h * scale_y,
        )


class Point(BaseModel):
    x: float
    y: float


class Poly(BaseModel):
    points: list[Point]
    enclosing_bb: BbF

    @classmethod
    def from_points(cls, points: list[Point]) -> "Poly":
        return cls(points=points, enclosing_bb=enclosing_bb(points))


def enclosing_bb(points: list[Point]) -> BbF:
    points = points
    min_x = math.inf
    min_y = math.inf
    max_x = -math.inf
    max_y = -math.inf
    for point in points:
        if point.x < min_x:
            min_x = point.x
        if point.y < min_y:
            min_y = point.y
        if point.x > max_x:
            max_x = point.x
        if point.y > max_y:
            max_y = point.y
    return BbF(x=min_x, y=min_y, w=max_x + 1 - min_x, h=max_y + 1 - min_y)


class CC:
    """Connected component"""

    def __init__(
        self,
        slices: tuple[slice, slice],
        label: int,
        im: np.ndarray,
        im_labeled: np.ndarray,
    ):
        self.im = im[slices].copy()
        self.im[im_labeled[slices] != label] = 0
        self.slices = slices
        self.bb = BbI.from_slices(slices)
        self.label = label

    def __str__(self):
        return "CC with " + str(self.bb)


def _find_cc_slices(im: np.ndarray):
    im_labeled, n_ccs = scpiy_label(im)  # type: ignore
    return find_objects(im_labeled), im_labeled, n_ccs


def find_ccs(im: np.ndarray) -> tuple[list[CC], np.ndarray]:
    """Find connected components in a binary image.
    Args:
        im: A binary image (2D numpy array) where connected components are to be found.
    Returns:
        A tuple containing:
            - A list of CC objects representing the connected components.
            - A labeled image where each connected component is assigned a unique label.
    """
    cc_slices, im_labeled, _ = _find_cc_slices(im)
    ccs = [CC(slc, i + 1, im, im_labeled) for i, slc in enumerate(cc_slices)]
    return ccs, im_labeled
