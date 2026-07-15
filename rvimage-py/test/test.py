from copy import deepcopy
import json

import cv2
import numpy as np
from pydantic import ValidationError
import pytest

from rvimage.collection_types import BboxAnnos, BrushAnnos, Canvas
from rvimage.converters import (
    decode_bytes_into_rgbarray,
    extract_polys_from_mask,
    fill_polys_on_mask,
    mask_to_rle,
    rle_to_mask,
)
from rvimage.coco import (
    RleOrder,
    coco_rle_to_mask,
    detect_rle_order,
    iter_rle_masks,
    mask_to_coco_rle,
)
from rvimage.domain import BbF, BbI, Point, Poly


def test_rle():
    im_mask = np.zeros((10, 10), dtype=np.uint8)
    im_mask[0:2, 0:5] = 1

    rle = mask_to_rle(im_mask)
    assert rle == [0, 5, 5, 5, 85]
    im_mask_converted = rle_to_mask(rle, 1, im_mask)
    assert np.array_equal(im_mask, im_mask_converted)


def test_polygon():
    im_mask = np.zeros((10, 10), dtype=np.uint8)
    polygons = [
        [
            Point(x=0, y=0),
            Point(x=5, y=0),
            Point(x=5, y=5),
            Point(x=2, y=7),
            Point(x=0, y=5),
        ]
    ]
    value = 1

    mask = fill_polys_on_mask(polygons, value, im_mask, abs_coords_input=True)
    assert np.sum(mask) > 0

    polygons_converted = extract_polys_from_mask(mask, abs_coords_output=True)
    assert len(polygons_converted) > 0
    im_mask = np.zeros((10, 10), dtype=np.uint8)
    mask_converted = fill_polys_on_mask(polygons, value, im_mask, abs_coords_input=True)

    assert np.allclose(mask, mask_converted), "Masks are not equal after conversion"


def test_validation():
    annos = {
        "elts": [],
        "cat_idxs": [],
        "selected_mask": [],
    }
    BboxAnnos.model_validate(annos)
    annos = {
        "elts": [{"BB": {"x": 0.0, "y": 0.0, "w": 5.0, "h": 5.0}}],
        "cat_idxs": [1],
        "selected_mask": [False],
    }
    BboxAnnos.model_validate(annos)


def test_inbox():
    bb1 = BbI(x=271, y=192, w=1014, h=86)
    bb2 = BbI(x=0, y=190, w=1500, h=100)
    assert bb1 in bb2

    with open("../rvimage/resources/test_data/rvprj_v4-0.json", "r") as f:
        data_loaded = json.load(f)

    def get_data(tool):
        for d, _ in data_loaded["tools_data_map"][tool]["specifics"][tool][
            "annotations_map"
        ].values():
            yield d

    data = get_data("Bbox")
    for i, bbox_data in enumerate(data):
        annos = BboxAnnos.model_validate(bbox_data)
        if i == 0:
            annos_ = deepcopy(annos)
            assert len(annos_.elts) == 4
            assert len(annos_.cat_idxs) == 4
            assert len(annos_.selected_mask) == 4
            assert len(list(annos.bbs())) == 4
            assert len(list(annos.bbs(cat_idx=[0]))) == 4
            assert len(list(annos.bbs(cat_idx=[1]))) == 0

            annos_.keep_only_inbox_annos(
                [
                    BbF(x=550.70, y=1300.28, w=1455, h=339),
                    BbF(x=105.72, y=416.41, w=327.09, h=932.01),
                ]
            )
            assert len(annos_.elts) == 2
            assert len(annos_.cat_idxs) == 2
            assert len(annos_.selected_mask) == 2
            assert len(list(annos_.bbs())) == 2

            annos.remove_inbox_annos(
                [
                    BbF(x=550.70, y=1300.28, w=1455, h=339),
                    BbF(x=105.72, y=416.41, w=327.09, h=932.01),
                ]
            )
            assert len(annos.elts) == 2
            assert len(annos.cat_idxs) == 2
            assert len(annos.selected_mask) == 2
            assert len(list(annos.bbs())) == 2

    data = get_data("Brush")
    for i, brush_data in enumerate(data):
        annos = BrushAnnos.model_validate(brush_data)
        if i == 0:
            annos_ = deepcopy(annos)
            assert len(annos_.elts) == 4
            assert len(annos_.cat_idxs) == 4
            assert len(annos_.selected_mask) == 4
            assert len(list(annos_.bbs())) == 4
            assert len(list(annos_.bbs(cat_idx=[0, 6]))) == 4
            assert len(list(annos_.bbs(cat_idx=[1, 3]))) == 0

            annos_.keep_only_inbox_annos([bb2])
            assert len(annos_.elts) == 1
            assert len(annos_.cat_idxs) == 1
            assert len(annos_.selected_mask) == 1
            assert len(list(annos_.bbs())) == 1

            annos.remove_inbox_annos([bb2])
            assert len(annos.elts) == 3
            assert len(annos.cat_idxs) == 3
            assert len(annos.selected_mask) == 3
            assert len(list(annos.bbs())) == 3


def test_from_mask():
    resulting_mask = np.zeros((64, 32), dtype=np.uint8)
    resulting_mask[31:40, 21:30] = 1
    BboxAnnos.from_mask(resulting_mask, 0)
    annos = BrushAnnos.from_mask(resulting_mask, 0)
    reconstructed_mask = np.zeros_like(resulting_mask)
    annos.fill_mask(reconstructed_mask, 0)
    assert np.array_equal(resulting_mask, reconstructed_mask), (
        "Reconstructed mask does not match the original mask"
    )


def test_len_validator():
    bb1 = BbF(x=11, y=0, w=10, h=20)
    with pytest.raises(ValidationError):
        BboxAnnos(elts=[bb1], cat_idxs=[], selected_mask=[])
    BboxAnnos(elts=[bb1], cat_idxs=[0], selected_mask=[True])
    rle = mask_to_rle(np.zeros((10, 20)))
    canvas = Canvas(rle=rle, bb=bb1.to_bbi(), intensity=1)
    with pytest.raises(ValidationError):
        BrushAnnos(elts=[canvas], cat_idxs=[], selected_mask=[])
    with pytest.raises(ValidationError):
        BrushAnnos(elts=[], cat_idxs=[1], selected_mask=[])
    BrushAnnos(elts=[], cat_idxs=[], selected_mask=[])
    BrushAnnos(elts=[canvas], cat_idxs=[1], selected_mask=[True])


def test_decode_image():
    bytes = open("../rvimage/resources/rvimage-logo.png", "rb").read()
    im_decoded = decode_bytes_into_rgbarray(bytes)
    im_read = cv2.imread("../rvimage/resources/rvimage-logo.png", cv2.IMREAD_COLOR)
    im_read = cv2.cvtColor(im_read, cv2.COLOR_BGR2RGB)
    assert im_decoded.shape == im_read.shape, (
        "Decoded image shape does not match read image shape"
    )
    assert np.array_equal(im_decoded, im_read), (
        "Decoded image does not match read image"
    )


def test_bb_rowcolinterface():
    def test(bb: BbI | BbF):
        assert bb.c_min == bb.x
        assert bb.r_min == bb.y
        assert bb.r_max == bb.y + bb.h
        assert bb.c_max == bb.x + bb.w
        assert bb.height == bb.h
        assert bb.width == bb.w
        if isinstance(bb, BbI):
            y_slice, x_slice = bb.slices
            assert y_slice == slice(bb.r_min, bb.r_max)
            assert x_slice == slice(bb.c_min, bb.c_max)

    test(BbF(x=0, y=0, w=10, h=20))
    test(BbI(x=0, y=0, w=1, h=2))
    test(BbF(x=0.5, y=0.76, w=10.8, h=20.22))
    test(BbI(x=1, y=120, w=11, h=21))


def test_intersect():
    bb1 = BbF(x=0, y=0, w=10, h=20)
    bb2 = BbI(x=0, y=0, w=10, h=20)
    # match the float box, not the int box
    assert bb1.intersect(bb2) == bb1
    assert bb1.intersect(bb2) != bb2

    # no overlap
    bb1 = BbF(x=11, y=0, w=10, h=20)
    bb4 = BbI(x=0, y=0, w=10, h=20)
    assert bb1.intersect(bb2) is None

    bb1 = BbF(x=5, y=6, w=10, h=20)
    bb3 = BbF(x=0, y=0, w=10, h=20)
    assert bb1.intersect(bb3) == BbF(x=5, y=6, w=5, h=14)

    bb1 = BbI(x=15, y=6, w=100, h=20)
    bb2 = BbI(x=20, y=10, w=100, h=120)
    assert bb1.intersect(bb2) == BbI(x=20, y=10, w=95, h=16)
    assert bb1.find_max_overlap_bb([bb1, bb2]) == bb1
    bb1 = BbI(x=15, y=6, w=100, h=20)
    bb2 = BbI(x=20, y=10, w=100, h=120)
    bbf = BbF(x=20, y=10, w=100, h=120)
    assert bbf.find_max_overlap_bb([bb1, bb2]) == bb2
    assert bb1.find_max_overlap_bb([bb2, bb3, bb4]) == bb2
    assert bb1.find_max_overlap_bb([bb4]) is None


def test_bb_conversion():
    bbi = BbI(x=0, y=0, w=10, h=20)
    assert BbF.from_bbi(bbi).to_bbi() == bbi

    bbf = BbF(x=0.04, y=0.4, w=9.54, h=20.3)
    assert bbf.to_bbi() == bbi
    assert bbf.to_bbi() == bbi.to_bbi()


def test_singleelt():
    bbf1 = BbF(x=0.04, y=0.4, w=9.54, h=20.3)
    bbf2 = BbF(x=10.47, y=0.7, w=3.54, h=7.3)
    bbf3 = BbF(x=8.14, y=6.2, w=8.54, h=10.3)
    bbf4 = BbF(x=8.14, y=6.2, w=8.54, h=10.3)
    annos = BboxAnnos.from_elt(bbf1, 0)
    annos.append_elt(bbf2, cat_idx=1)
    annos.append_elt(bbf3, cat_idx=3)
    annos.append_elt(bbf4, cat_idx=3)
    annos_ref = BboxAnnos(
        elts=[bbf1, bbf2, bbf3], cat_idxs=[0, 1, 3], selected_mask=[False, False, False]
    )
    assert annos == annos_ref

    # with a different category, it is not a duplicate
    annos.append_elt(bbf4, cat_idx=4)
    annos_ref = BboxAnnos(
        elts=[bbf1, bbf2, bbf3, bbf4],
        cat_idxs=[0, 1, 3, 4],
        selected_mask=[False, False, False, False],
    )
    assert annos == annos_ref
    poly1 = Poly.from_points(
        [Point(x=1.2, y=1.3), Point(x=2.2, y=1.6), Point(x=11.2, y=10.3)]
    )
    annos.append_elt(poly1, cat_idx=4)
    annos_ref = BboxAnnos(
        elts=[bbf1, bbf2, bbf3, bbf4, poly1],
        cat_idxs=[0, 1, 3, 4, 4],
        selected_mask=[False, False, False, False, False],
    )
    assert annos == annos_ref

    annos.extend(annos_ref)
    annos_ref = BboxAnnos(
        elts=[bbf1, bbf2, bbf3, bbf4, poly1],
        cat_idxs=[0, 1, 3, 4, 4],
        selected_mask=[
            False,
            False,
            False,
            False,
            False,
        ],
    )
    assert annos == annos_ref
    annos_ = BboxAnnos.from_elt(bbf1, 6)
    annos_.append_elt(bbf2, 7, True)
    annos.extend(annos_)
    annos_ref = BboxAnnos(
        elts=[bbf1, bbf2, bbf3, bbf4, poly1, bbf1, bbf2],
        cat_idxs=[0, 1, 3, 4, 4, 6, 7],
        selected_mask=[False, False, False, False, False, False, True],
    )
    assert annos == annos_ref


def test_equals():
    bbf1 = BbF(x=0.04, y=0.4, w=9.54, h=20.3)
    bbf2 = BbF(x=10.47, y=0.7, w=3.54, h=7.3)
    bbf3 = BbF(x=8.14, y=6.2, w=8.54, h=10.3)
    bbf4 = BbF(x=8.14, y=6.2, w=8.54, h=10.3)
    bbf5 = BbF(x=8.14 + 1e-9, y=6.2, w=8.54, h=10.3)
    assert not bbf1.equals(bbf2)
    assert bbf3.equals(bbf4)
    assert bbf3.equals(bbf5)

    poly1 = Poly.from_points(
        [Point(x=1.2, y=1.3), Point(x=2.2, y=1.6), Point(x=11.2, y=10.3)]
    )
    poly2 = Poly.from_points(
        [Point(x=1.2, y=1.3), Point(x=2.2, y=1.6), Point(x=11.2, y=10.3)]
    )
    poly3 = Poly.from_points(
        [Point(x=2.2, y=1.3), Point(x=4.2, y=4.6), Point(x=11.2, y=19.3)]
    )
    poly4 = Poly.from_points(
        [Point(x=2.2, y=1.3 + 1e-9), Point(x=4.2, y=4.6), Point(x=11.2, y=19.3)]
    )
    assert poly1.equals(poly2)
    assert not poly1.equals(poly3)
    assert poly3.equals(poly4)


def _rle_of(flat):
    counts = []
    run = 0
    val = 0
    for v in flat:
        if v == val:
            run += 1
        else:
            counts.append(run)
            run = 1
            val = int(v)
    counts.append(run)
    return counts


def test_coco_rle_roundtrip_nonsquare():
    # non-square: height=4, width=6
    truth = np.zeros((4, 6), dtype=np.uint8)
    truth[1, 2] = 1
    truth[2, 2] = 1
    truth[2, 3] = 1
    truth[0, 5] = 1
    truth[3, 0] = 1

    seg = mask_to_coco_rle(truth)
    assert seg["size"] == [4, 6]  # coco: [height, width]
    decoded = coco_rle_to_mask(seg["counts"], seg["size"])
    assert decoded.shape == truth.shape
    assert np.array_equal(decoded, truth)


def test_coco_rle_order_detection():
    truth = np.zeros((4, 6), dtype=np.uint8)
    truth[1, 2] = 1
    truth[2, 3] = 1
    truth[3, 0] = 1

    # legacy RV export: row-major counts, size = [width, height], RV signature
    old_rv = dict(
        counts=_rle_of(truth.flatten(order="C")),
        size=[6, 4],
        info={"description": "created with RV Image Version v0.3.3"},
    )
    # new RV export: column-major, size = [height, width], explicit marker
    new_rv = dict(
        counts=_rle_of(truth.flatten(order="F")),
        size=[4, 6],
        info={
            "description": "created with RV Image Version v0.8.0",
            "rvimage_rle_order": "column_major",
        },
    )
    # external coco: column-major, size = [height, width], no signature
    external = dict(
        counts=_rle_of(truth.flatten(order="F")),
        size=[4, 6],
        info={"description": "Created by BASF"},
    )

    assert detect_rle_order(old_rv["info"]) == RleOrder.ROW_MAJOR
    assert detect_rle_order(new_rv["info"]) == RleOrder.COLUMN_MAJOR
    assert detect_rle_order(external["info"]) == RleOrder.COLUMN_MAJOR
    assert detect_rle_order(None) == RleOrder.COLUMN_MAJOR

    for f in (old_rv, new_rv, external):
        order = detect_rle_order(f["info"])
        mask = coco_rle_to_mask(f["counts"], f["size"], order)
        assert mask.shape == truth.shape
        assert np.array_equal(mask, truth)


def test_iter_rle_masks():
    truth = np.zeros((4, 6), dtype=np.uint8)
    truth[0, 0] = 1
    truth[3, 5] = 1
    coco = {
        "info": {"rvimage_rle_order": "column_major"},
        "annotations": [
            {"id": 1, "segmentation": mask_to_coco_rle(truth)},
            {"id": 2, "segmentation": [[0.0, 0.0, 1.0, 1.0]]},  # polygon: skipped
        ],
    }
    results = list(iter_rle_masks(coco))
    assert len(results) == 1
    ann, mask = results[0]
    assert ann["id"] == 1
    assert np.array_equal(mask, truth)


if __name__ == "__main__":
    test_len_validator()
    test_equals()
    test_singleelt()
    test_bb_conversion()
    test_intersect()
    test_inbox()
    test_bb_rowcolinterface()
    test_from_mask()
    test_decode_image()
    test_validation()
    test_rle()
    test_polygon()
    test_coco_rle_roundtrip_nonsquare()
    test_coco_rle_order_detection()
    test_iter_rle_masks()
