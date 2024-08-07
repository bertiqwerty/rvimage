import rvimage
import numpy as np


def rle_to_mask(im_mask: np.ndarray, rle: list[int]) -> np.ndarray:
    shape = im_mask.shape[:2]
    flat_mask = im_mask.ravel()
    pos = 0
    for i, n_elts in enumerate(rle):
        if i % 2 == 1:
            flat_mask[pos : pos + n_elts] = 1
        pos = pos + n_elts
    return flat_mask.reshape(shape)


def test():
    rle_list = [
        6585504,
        15,
        4303,
        19,
        4299,
        23,
        4296,
        25,
        4294,
        27,
        4292,
        29,
        4291,
        29,
        4290,
        31,
        4289,
        31,
        4288,
        33,
        4287,
        33,
        4287,
        33,
        4287,
        33,
        4287,
        33,
        4287,
        33,
        4287,
        33,
        4288,
        32,
        4288,
        33,
        4288,
        32,
        4288,
        33,
        4288,
        32,
        4289,
        32,
        4289,
        31,
        4291,
        30,
        4292,
        28,
        4293,
        27,
        4293,
        27,
        4294,
        26,
        4294,
        26,
        4294,
        26,
        4294,
        26,
        4294,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        26,
        4294,
        26,
        4294,
        27,
        4293,
        27,
        4293,
        28,
        4292,
        28,
        4293,
        28,
        4292,
        28,
        4293,
        27,
        4293,
        27,
        4294,
        26,
        4294,
        26,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        26,
        4294,
        26,
        4294,
        27,
        4293,
        27,
        4293,
        28,
        4292,
        28,
        4293,
        28,
        4292,
        28,
        4293,
        27,
        4293,
        27,
        4294,
        26,
        4294,
        26,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        26,
        4294,
        26,
        4294,
        27,
        4293,
        27,
        4293,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4293,
        27,
        4293,
        27,
        4294,
        26,
        4294,
        26,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        26,
        4294,
        25,
        4295,
        25,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        26,
        4294,
        26,
        4294,
        26,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        29,
        4291,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        29,
        4291,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4294,
        26,
        4294,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4295,
        25,
        4294,
        26,
        4294,
        26,
        4293,
        27,
        4293,
        27,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4292,
        27,
        4292,
        28,
        4292,
        27,
        4292,
        28,
        4292,
        28,
        4291,
        29,
        4291,
        29,
        4290,
        30,
        4290,
        30,
        4289,
        30,
        4289,
        31,
        4288,
        31,
        4288,
        32,
        4288,
        31,
        4288,
        31,
        4289,
        30,
        4289,
        30,
        4290,
        29,
        4291,
        29,
        4291,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4291,
        28,
        4292,
        28,
        4291,
        28,
        4292,
        28,
        4291,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        28,
        4292,
        27,
        4293,
        27,
        4293,
        26,
        4295,
        25,
        4295,
        24,
        4297,
        22,
        4298,
        21,
        4300,
        19,
        4302,
        17,
        4304,
        15,
        4307,
        11,
        4311,
        7,
        19144426,
    ]
    rle = np.array(rle_list, dtype=np.uint32)
    w, h = 4320, 6496
    mask1 = np.zeros((h, w), dtype=np.uint8)
    mask2 = np.zeros((h, w), dtype=np.uint8)
    rvimage.rle_to_mask_inplace(rle, mask1, w)
    mask2 = rle_to_mask(mask2, rle_list)
    assert mask1.sum() == mask2.sum()
    assert mask1.shape == mask2.shape
    assert np.allclose(mask1, mask2)


if __name__ == "__main__":
    test()
