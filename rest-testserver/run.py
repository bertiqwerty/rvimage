import json
from typing import Annotated

import numpy as np
from fastapi import FastAPI, File, Form, Query, UploadFile
from pydantic import TypeAdapter
from rvimage.collection_types import (
    BboxAnnos,
    BrushAnnos,
    InputAnnotationData,
    OutputAnnotationData,
)
from rvimage.converters import decode_bytes_into_rgbarray
from rvimage.domain import BbF

app = FastAPI()


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict", response_model=OutputAnnotationData)
async def predict(
    image: Annotated[UploadFile, File(...)],
    parameters: Annotated[str, Form(...)],
    input_annotations: Annotated[str, Form(...)],
    zoom_box: Annotated[str, Form],
    active_tool: Annotated[str, Query()],
) -> OutputAnnotationData:
    bytes = await image.read()
    im = decode_bytes_into_rgbarray(bytes)
    print(f"image shape {im.shape}")
    zb = TypeAdapter(BbF | None).validate_json(zoom_box)
    print(zb)
    data = InputAnnotationData.model_validate_json(input_annotations)
    parameters = json.loads(parameters)
    bbd = data.bbox
    brd = data.brush
    resulting_mask = np.zeros(im.shape[:2], dtype=np.uint8)
    resulting_mask[31:40, 21:30] = 1
    bbox_annos = BboxAnnos.from_mask(resulting_mask, 0)
    resulting_mask = np.zeros(im.shape[:2], dtype=np.uint8)
    resulting_mask[30, 23:26] = 1
    resulting_mask[76:80, 5] = 1
    brush_annos = BrushAnnos.from_mask(resulting_mask, 0)

    bbox_annos = bbox_annos.extend(None if bbd is None else bbd.annos)
    brush_annos = brush_annos.extend(None if brd is None else brd.annos)

    oad = OutputAnnotationData(
        bbox=bbox_annos,
        brush=brush_annos,
    )
    return oad
