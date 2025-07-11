from typing import Self
from pydantic import BaseModel, model_serializer, model_validator
import numpy as np
from rvimage.converters import (
    extract_polys_from_mask,
    fill_bbs_on_mask,
    fill_polys_on_mask,
    mask_to_rle,
    rle_to_mask,
)
from rvimage.domain import BbF, BbI, Poly, find_ccs


class Labelinfo(BaseModel):
    new_label: str
    labels: list[str]
    colors: list[list[int]]
    cat_ids: list[int]
    cat_idx_current: int
    show_only_current: bool


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

    def extend(self, other: Self | None) -> "BboxAnnos":
        """
        Extend the current BboxAnnos with another BboxAnnos.
        """
        if other is None:
            return self
        return BboxAnnos(
            elts=self.elts + other.elts,
            cat_idxs=self.cat_idxs + other.cat_idxs,
            selected_mask=self.selected_mask + other.selected_mask,
        )

    def fill_mask(self, im_mask: np.ndarray, cat_idx: int):
        fill_polys_on_mask(
            polygons=(
                elt.points
                for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
                if cat_idx == cat_idx_ and isinstance(elt, Poly)
            ),
            value=1,
            im_mask=im_mask,
            abs_coords_input=True,
        )
        fill_bbs_on_mask(
            bbs=(
                elt
                for elt, cat_idx_ in zip(self.elts, self.cat_idxs)
                if cat_idx == cat_idx_ and isinstance(elt, BbF)
            ),
            value=1,
            im_mask=im_mask,
            abs_coords_input=True,
        )


class BboxData(BaseModel):
    annos: BboxAnnos
    labelinfo: Labelinfo


class Canvas(BaseModel):
    rle: list[int]
    bb: BbI
    intensity: float


class BrushAnnos(BaseModel):
    elts: list[Canvas]
    cat_idxs: list[int]
    selected_mask: list[bool]

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

    def fill_mask(self, im_mask: np.ndarray, cat_idx: int):
        """Create a binary mask from all brush annotations of the given category

        Args:
            im_mask: output mask to write on
            cat_idx: index of category to be written
        """
        for elt, cat_idx_ in zip(self.elts, self.cat_idxs):
            if cat_idx == cat_idx_:
                im_bb_mask = rle_to_mask(elt.rle, value=1, mask=elt.bb)
                im_mask[elt.bb.slices] = im_bb_mask

    def extend(self, other: Self | None) -> "BrushAnnos":
        """
        Extend the current BrushAnnos with another BrushAnnos.
        """
        if other is None:
            return self
        return BrushAnnos(
            elts=self.elts + other.elts,
            cat_idxs=self.cat_idxs + other.cat_idxs,
            selected_mask=self.selected_mask + other.selected_mask,
        )


class BrushData(BaseModel):
    annos: BrushAnnos
    labelinfo: Labelinfo


class InputAnnotationData(BaseModel):
    bbox: BboxData | None
    brush: BrushData | None


class OutputAnnotationData(BaseModel):
    bbox: BboxAnnos | None
    brush: BrushAnnos | None
