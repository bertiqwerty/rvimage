import json
import numpy as np
from rvimage.collection_types import BboxAnnos, BrushAnnos
from rvimage.converters import extract_polys_from_mask, fill_polys_on_mask
from rvimage.converters import rle_to_mask, mask_to_rle
from rvimage.domain import Point


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
    print(mask)
    assert np.sum(mask) > 0

    polygons_converted = extract_polys_from_mask(mask, abs_coords_output=True)
    assert len(polygons_converted) > 0
    im_mask = np.zeros((10, 10), dtype=np.uint8)
    mask_converted = fill_polys_on_mask(polygons, value, im_mask, abs_coords_input=True)

    assert np.allclose(mask, mask_converted), "Masks are not equal after conversion"


def test_validation():
    annos = {
        "elts": [{"BB": {"x": 0.0, "y": 0.0, "w": 5.0, "h": 5.0}}],
        "cat_idxs": [1],
        "selected_mask": [False],
    }
    BboxAnnos.model_validate(annos)
    with open("../rvimage/resources/test_data/rvprj_v4-0.json", "r") as f:
        data_loaded = json.load(f)

    def get_data(tool):
        for d, _ in data_loaded["tools_data_map"][tool]["specifics"][tool][
            "annotations_map"
        ].values():
            yield d

    for brush_data in get_data("Brush"):
        BrushAnnos.model_validate(brush_data)
    for bbox_data in get_data("Bbox"):
        BboxAnnos.model_validate(bbox_data)


if __name__ == "__main__":
    test_rle()
    test_polygon()
