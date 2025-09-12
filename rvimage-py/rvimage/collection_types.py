from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Protocol, Self, TypeVar
from collections.abc import Iterable

import numpy as np
from pydantic import BaseModel, model_serializer, model_validator

from rvimage.converters import (
    extract_polys_from_mask,
    fill_bbs_on_mask,
    fill_polys_on_mask,
    mask_to_rle,
    rle_to_mask,
)
from rvimage.domain import BbF, BbI, Poly, find_ccs

B = TypeVar("B", BbI, BbF)
T = TypeVar("T")


def _inbox_inds(
    container_boxes: Sequence[BbF | BbI],
    keep_or_remove: Sequence[T],
    convert_to_box: Callable[[T], B],
):
    return [
        i
        for i, elt in enumerate(keep_or_remove)
        if any(convert_to_box(elt) in cb for cb in container_boxes)
    ]


def _outofbox_inds(
    container_boxes: Sequence[BbF | BbI],
    keep_or_remove: Sequence[T],
    convert_to_box: Callable[[T], B],
):
    return [
        i
        for i, elt in enumerate(keep_or_remove)
        if all(convert_to_box(elt) not in cb for cb in container_boxes)
    ]


class _AnnotationProtocol(Protocol):
    elts: list
    cat_idxs: list[int]
    selected_mask: list[bool]


def _keep_inds(obj: _AnnotationProtocol, inds: list[int]):
    obj.elts = [obj.elts[i] for i in inds]
    obj.cat_idxs = [obj.cat_idxs[i] for i in inds]
    obj.selected_mask = [obj.selected_mask[i] for i in inds]


class Labelinfo(BaseModel):
    new_label: str
    labels: list[str]
    colors: list[list[int]]
    cat_ids: list[int]
    cat_idx_current: int
    show_only_current: bool


def _bbox_elt_to_bb(elt: BbF | Poly) -> BbF:
    return elt if isinstance(elt, BbF) else elt.enclosing_bb


class BboxAnnos(BaseModel):
    elts: list[BbF | Poly]
    cat_idxs: list[int]
    selected_mask: list[bool]

    @model_validator(mode="before")
    @classmethod
    def resolve_bb_poly(cls, data: dict) -> dict:
        # remove the type-info from the dict
        if len(data["elts"]) > 0 and isinstance(data["elts"][0], dict):
            data["elts"] = [next(v for v in d.values()) for d in data["elts"]]
        return data

    @model_validator(mode="after")
    def check_len(self) -> "Self":
        assert len(self.elts) == len(self.cat_idxs) == len(self.selected_mask)
        return self

    @model_serializer()
    def serialize_model(self):
        elts = [
            {"BB": elt.model_dump()}
            if isinstance(elt, BbF)
            else {"Poly": elt.model_dump()}
            for elt in self.elts
        ]
        return {
            "cat_idxs": self.cat_idxs,
            "selected_mask": self.selected_mask,
            "elts": elts,
        }

    @classmethod
    def from_mask(cls, mask: np.ndarray, cat_idx: int) -> "BboxAnnos":
        """
        Create BboxAnnos from a binary mask.
        """
        polys = extract_polys_from_mask(mask, abs_coords_output=True)
        cat_idxs = [cat_idx] * len(polys)

        return cls(
            elts=[Poly.from_points(points) for points in polys],
            cat_idxs=cat_idxs,
            selected_mask=[False] * len(polys),
        )

    def extend(self, other: Self | None):
        """
        Extend the current BboxAnnos with another BboxAnnos.
        """
        if other is None:
            return self
        for o_elt, o_cat_idx, o_selected in zip(
            other.elts, other.cat_idxs, other.selected_mask
        ):
            self.append_elt(o_elt, o_cat_idx, o_selected)

    def append_elt(
        self, elt: BbI | BbF | Poly, cat_idx: int, selected_mask: bool = False
    ):
        if isinstance(elt, BbI):
            elt = BbF.from_bbi(elt)
        is_duplicate = any(
            elt.equals(elt_i) and cat_idx == cat_idx_i
            for elt_i, cat_idx_i in zip(self.elts, self.cat_idxs)
        )
        if not is_duplicate:
            self.elts.append(elt)
            self.cat_idxs.append(cat_idx)
            self.selected_mask.append(selected_mask)

    @classmethod
    def from_elt(cls, elt: BbI | BbF | Poly, cat_idx: int) -> "BboxAnnos":
        if isinstance(elt, BbI):
            elt = BbF.from_bbi(elt)
        return cls(elts=[elt], cat_idxs=[cat_idx], selected_mask=[False])

    def fill_mask(self, im_mask: np.ndarray, cat_idx: int, value: int = 1):
        fill_polys_on_mask(
            polygons=(
                elt.points
                for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
                if cat_idx == cat_idx_ and isinstance(elt, Poly)
            ),
            value=value,
            im_mask=im_mask,
            abs_coords_input=True,
        )
        fill_bbs_on_mask(
            bbs=(
                elt
                for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
                if cat_idx == cat_idx_ and isinstance(elt, BbF)
            ),
            value=value,
            im_mask=im_mask,
            abs_coords_input=True,
        )

    def bbs(self, cat_idx: list[int] | None = None) -> Iterable[BbF]:
        return (
            elt if isinstance(elt, BbF) else elt.enclosing_bb
            for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
            if cat_idx is None or cat_idx_ in cat_idx
        )

    def keep_only_inbox_annos(self, bbs: Sequence[BbF | BbI]):
        """Keep all annotations whose bounding box is contained in one of the passed bbs"""
        inds = _inbox_inds(
            bbs,
            self.elts,
            _bbox_elt_to_bb,
        )
        _keep_inds(self, inds)

    def remove_inbox_annos(self, bbs: Sequence[BbF | BbI]):
        """Remove all annotations whose bounding box is contained in one of the passed bbs"""
        inds = _outofbox_inds(
            bbs,
            self.elts,
            lambda elt: elt if isinstance(elt, BbF) else elt.enclosing_bb,
        )
        _keep_inds(self, inds)

    def find_max_overlap_bb_with_zoombox(self, zb: BbI, cat_idxs: list[int]) -> BbI:
        if len(self.elts) > 0:
            max_ol_bb = zb.find_max_overlap_bb(self.bbs(cat_idxs))
            if max_ol_bb is None:
                return zb
            else:
                return max_ol_bb.to_bbi()
        else:
            return zb


class BboxData(BaseModel):
    annos: BboxAnnos
    labelinfo: Labelinfo


class Canvas(BaseModel):
    rle: list[int]
    bb: BbI
    intensity: float


def _brush_elt_to_bb(elt: Canvas) -> BbI:
    return elt.bb


class BrushAnnos(BaseModel):
    elts: list[Canvas]
    cat_idxs: list[int]
    selected_mask: list[bool]

    @model_validator(mode="after")
    def check_len(self) -> "Self":
        assert len(self.elts) == len(self.cat_idxs) == len(self.selected_mask)
        return self

    @classmethod
    def from_mask(cls, im_mask: np.ndarray, cat_idx: int) -> "BrushAnnos":
        """
        Create BrushAnnos from a binary mask.
        """
        ccs, _ = find_ccs(im_mask)
        rles = [mask_to_rle(cc.im) for cc in ccs]

        return BrushAnnos(
            elts=[
                Canvas(rle=rle, bb=cc.bb, intensity=1.0) for rle, cc in zip(rles, ccs)
            ],
            cat_idxs=[cat_idx] * len(ccs),
            selected_mask=[False] * len(ccs),
        )

    def fill_mask(self, im_mask: np.ndarray, cat_idx: int, value: int = 1):
        """Create a binary mask from all brush annotations of the given category

        Args:
            im_mask: output mask to write on
            cat_idx: index of category to be written
            value: image intensity of masked pixels
        """
        for elt, cat_idx_ in zip(self.elts, self.cat_idxs):
            if cat_idx == cat_idx_:
                im_bb_mask = rle_to_mask(elt.rle, value=value, mask=elt.bb)
                im_mask[elt.bb.slices][im_bb_mask > 0] = im_bb_mask[im_bb_mask > 0]

    def extend(self, other: Self | None):
        """
        Extend the current BrushAnnos with another BrushAnnos.
        """
        if other is None:
            return self
        self.elts += other.elts
        self.cat_idxs += other.cat_idxs
        self.selected_mask += other.selected_mask

    def bbs(self, cat_idx: list[int] | None = None) -> Iterable[BbI]:
        return (
            _brush_elt_to_bb(elt)
            for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
            if cat_idx is None or cat_idx_ in cat_idx
        )

    def keep_only_inbox_annos(self, bbs: Sequence[BbI | BbF]):
        """Keep all annotations whose bounding box is contained in of the passed bbs"""
        inds = _inbox_inds(bbs, self.elts, _brush_elt_to_bb)
        _keep_inds(self, inds)

    def remove_inbox_annos(self, bbs: Sequence[BbI | BbF]):
        """Keep all annotations whose bounding box is contained in of the passed bbs"""
        inds = _outofbox_inds(bbs, self.elts, _brush_elt_to_bb)
        _keep_inds(self, inds)


class BrushData(BaseModel):
    annos: BrushAnnos
    labelinfo: Labelinfo


class InputAnnotationData(BaseModel):
    bbox: BboxData | None
    brush: BrushData | None


class OutputAnnotationData(BaseModel):
    bbox: BboxAnnos | None
    brush: BrushAnnos | None
