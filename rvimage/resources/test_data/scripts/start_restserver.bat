rem used by unit test on windows in rvimage/src/rvlib/tools/wand.rs
set PYTHONPATH=../rvimage-py
cd %1/../rest-testserver
uv sync
uv run fastapi run run.py&