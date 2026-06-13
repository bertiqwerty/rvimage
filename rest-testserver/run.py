import json
from typing import Annotated

import numpy as np
from fastapi import FastAPI, File, Form, Query, Request, UploadFile, status
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from loguru import logger
from pydantic import TypeAdapter, BaseModel, model_validator

from rvimage.collection_types import (
    BboxAnnos,
    BrushAnnos,
    InputAnnotationData,
    OutputAnnotationData,
    WandManyMessage,
    validate_params,
)
from rvimage.converters import decode_bytes_into_rgbarray
from rvimage.domain import BbF

app = FastAPI()


@app.exception_handler(RequestValidationError)
async def validation_exception_handler(request: Request, exc: RequestValidationError):
    exc_str = f"{exc}".replace("\n", " ").replace("   ", " ")
    logger.error(exc_str)
    content = {"status_code": 10422, "message": exc_str, "data": None}
    return JSONResponse(
        content=content, status_code=status.HTTP_422_UNPROCESSABLE_ENTITY
    )


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict", response_model=OutputAnnotationData)
async def predict(
    image: Annotated[UploadFile, File(...)],
    parameters: Annotated[str, Form(...)],
    input_annotations: Annotated[str, Form(...)],
    zoom_box: Annotated[str, Form(...)],
    active_tool: Annotated[str, Query()],
) -> OutputAnnotationData:
    bytes = await image.read()
    im = decode_bytes_into_rgbarray(bytes)
    print(f"image shape {im.shape}")
    zb = TypeAdapter(BbF | None).validate_json(zoom_box)
    print(zb)
    print(input_annotations)
    data = InputAnnotationData.model_validate_json(input_annotations)
    parameters = json.loads(parameters)
    bbd = data.bbox
    brd = data.brush
    resulting_mask = np.zeros(im.shape[:2], dtype=np.uint8)
    resulting_mask[31:40, 21:30] = 1
    print("bbox from mask")
    bbox_annos = BboxAnnos.from_mask(resulting_mask, 0)
    resulting_mask = np.zeros(im.shape[:2], dtype=np.uint8)
    resulting_mask[30, 23:26] = 1
    resulting_mask[76:80, 5] = 1
    print("brush from mask")
    brush_annos = BrushAnnos.from_mask(resulting_mask, 0)

    logger.info(brush_annos)
    logger.info(bbox_annos)
    bbox_annos.extend(None if bbd is None else bbd.annos)
    brush_annos.extend(None if brd is None else brd.annos)

    oad = OutputAnnotationData(
        bbox=bbox_annos,
        brush=brush_annos,
    )
    return oad


class DummyParams(BaseModel):
    a: int | None = None
    b: str | None = None

    @model_validator(mode="before")
    @classmethod
    def validate(cls, v):
        return validate_params(v)


@app.post("/predict_many")
async def predict_many(
    prj_name: Annotated[str, Form(...)],
    input_annotations: Annotated[str, Form(...)],
    files: Annotated[str, Form(...)],
    communication: Annotated[str, Form(...)],
    parameters: Annotated[str, Form(...)],
) -> tuple[OutputAnnotationData, str]:
    InputAnnotationData.model_validate_json(input_annotations)
    file_list = json.loads(files)
    comms: list[WandManyMessage] = [
        WandManyMessage.model_validate(m) for m in json.loads(communication)
    ]
    params = DummyParams.model_validate_json(parameters)
    print(f"project name: {prj_name}")
    print(f"files: {file_list}")
    print(f"communication: {comms}")
    print(f"parameters: {params}")

    return (OutputAnnotationData(bbox=None, brush=None), "method_description")
