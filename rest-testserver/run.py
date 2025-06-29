import json
from typing import Annotated
from fastapi import FastAPI, File, Form, Query, UploadFile
import cv2
import numpy as np
from rvimage.collection_types import (
    InputAnnotationData,
    OutputAnnotationData,
    BrushAnnos,
    BboxAnnos,
)

app = FastAPI()


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict", response_model=OutputAnnotationData)
async def predict(
    image: Annotated[UploadFile, File(...)],
    parameters: Annotated[str, Form(...)],
    input_annotations: Annotated[str, Form(...)],
    active_tool: Annotated[str, Query()],
) -> OutputAnnotationData:
    bytes = await image.read()
    np_bytes = np.frombuffer(bytes, np.uint8)
    im = cv2.imdecode(np_bytes, cv2.IMREAD_COLOR)  # BGR format
    im = cv2.cvtColor(im, cv2.COLOR_BGR2RGB)
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
