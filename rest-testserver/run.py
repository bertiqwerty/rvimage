import json
from typing import Annotated
from fastapi import FastAPI, File, Form, Query, UploadFile
from pydantic import BaseModel
import cv2
import numpy as np

app = FastAPI()


class AttrVal(BaseModel):
    pass


class BbF(BaseModel):
    x: float
    y: float
    w: float
    h: float


class Point(BaseModel):
    x: float
    y: float


class EnclosingBb(BaseModel):
    x: float
    y: float
    w: float
    h: float


class Poly(BaseModel):
    points: list[Point]
    enclosing_bb: EnclosingBb


class GeoFig(BaseModel):
    bbox: BbF | None = None
    poly: Poly | None = None


class BboxAnnos(BaseModel):
    elts: list[GeoFig]
    cat_idxs: list[int]
    selected_mask: list[bool]


class Labelinfo(BaseModel):
    new_label: str
    labels: list[str]
    colors: list[list[int]]
    cat_ids: list[int]
    cat_idx_current: int
    show_only_current: bool


class BboxData(BaseModel):
    annos: BboxAnnos
    labelinfo: Labelinfo


class BbI(BaseModel):
    x: int
    y: int
    w: int
    h: int


class Canvas(BaseModel):
    rle: list[int]
    bb: BbI
    intensity: float


class BrushAnnos(BaseModel):
    elts: list[Canvas]
    cat_idxs: list[int]
    selected_mask: list[bool]


class BrushData(BaseModel):
    annos: BrushAnnos
    labelinfo: Labelinfo


class InputAnnotationData(BaseModel):
    bbox: BboxData
    brush: BrushData


class OutputAnnotationData(BaseModel):
    bbox: BboxAnnos
    brush: BrushAnnos


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict")
async def predict(
    image: Annotated[UploadFile, File(...)],
    parameters: Annotated[str, Form(...)],
    input_annotations: Annotated[str, Form(...)],
    active_tool: Annotated[str, Query()],
):
    bytes = await image.read()
    np_bytes = np.frombuffer(bytes, np.uint8)
    im = cv2.imdecode(np_bytes, cv2.IMREAD_COLOR)  # BGR format
    im = cv2.cvtColor(im, cv2.COLOR_BGR2RGB)
    print(f"Image shape: {im.shape}")  # Debugging line
    print(f"active tool name {active_tool}")
    cv2.imwrite("test.jpg", im)  # Save the image for debugging
    data = InputAnnotationData.model_validate_json(input_annotations)
    parameters = json.loads(parameters)
    return OutputAnnotationData(bbox=data.bbox.annos, brush=data.brush.annos)
