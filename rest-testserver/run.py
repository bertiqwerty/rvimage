import json
from typing import Annotated
from fastapi import FastAPI, File, Form, Query, UploadFile
import cv2
import numpy as np
from rvimage.collection_types import InputAnnotationData, OutputAnnotationData

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
    print(f"Image shape: {im.shape}")  # Debugging line
    print(f"active tool name {active_tool}")
    print(input_annotations)
    data = InputAnnotationData.model_validate_json(input_annotations)
    parameters = json.loads(parameters)
    bbd = data.bbox
    brd = data.brush
    oad = OutputAnnotationData(
        bbox=None if bbd is None else bbd.annos,
        brush=None if brd is None else brd.annos,
    )
    print("OAD")
    print(oad)
    return oad
