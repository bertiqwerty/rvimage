from fastapi import FastAPI

app = FastAPI()


@app.get("/ping")
async def ping():
    return "pong"


@app.post("/predict")
async def predict():
    return "pong"