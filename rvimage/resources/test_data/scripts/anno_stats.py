# /// script
# requires-python = ">=3.11"
# dependencies = ["rvimage"]
#
# [tool.uv.sources]
# rvimage = { path = "../../../../rvimage-py", editable = true }
# ///
"""Annotation statistics command used by rvimage's unit tests."""

import os
import sys
from pathlib import Path

from rvimage.collection_types import StatsCmdInput, StatsTable


def main():
    data = StatsCmdInput.model_validate_json(sys.stdin.read())
    images = data.images or {}
    rows = []
    for tool, tool_annos in (
        ("bbox", data.annotations.bbox),
        ("brush", data.annotations.brush),
    ):
        if tool_annos is None:
            continue
        for file, annos in tool_annos.annos:
            image = images.get(file)
            image_bytes = os.path.getsize(image) if image is not None else None
            rows.append([Path(file).stem, tool, len(annos.elts), image_bytes])
    table = StatsTable(
        columns=["file", "tool", "# annotations", "image bytes"],
        data=rows,
        column_hints=[None, None, None, "size of the image file"],
    )
    sys.stdout.write(table.model_dump_json())


if __name__ == "__main__":
    main()
