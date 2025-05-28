from collections.abc import Sequence
from datetime import datetime
import json
from typing import Annotated
from fastapi import FastAPI, File, Form, Query, UploadFile
from pydantic import BaseModel
import cv2
import numpy as np

app = FastAPI()


class Info(BaseModel):
    year: int | None = None
    version: str | None = None
    description: str | None = None
    contributor: str | None = None
    url: str | None = None
    date_created: datetime | None = None


class Image(BaseModel):
    id: int
    width: int
    height: int
    file_name: str
    license: int | None = None
    flickr_url: str | None = None
    coco_url: str | None = None
    date_captured: datetime | None = None


class License(BaseModel):
    id: int
    name: str
    url: str


class RLE(BaseModel):
    counts: Sequence[int]
    size: tuple[int, int]
    intensity: float | None = None


class Annotation(BaseModel):
    id: int
    image_id: int
    category_id: int
    bbox: Sequence[float]
    segmentation: RLE | Sequence[Sequence[float]]
    area: float | None = None


class Category(BaseModel):
    id: int
    name: int


class Coco(BaseModel):
    info: Info
    images: Sequence[Image]
    annotations: Sequence[Annotation]
    categories: Sequence[Category]


class AttrVal(BaseModel):
    pass


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict")
async def predict(
    image: Annotated[UploadFile, File(...)],
    parameters: Annotated[str, Form(...)],
    annotations: Annotated[str, Form(...)],
    label_names: Annotated[list[str] | None, Query()] = None,
):
    assert label_names == ["some_label"]
    bytes = await image.read()
    np_bytes = np.frombuffer(bytes, np.uint8)
    im = cv2.imdecode(np_bytes, cv2.IMREAD_COLOR)  # BGR format
    im = cv2.cvtColor(im, cv2.COLOR_BGR2RGB)
    coco = Coco.model_validate_json(annotations)
    parameters = json.loads(parameters)
    return coco
