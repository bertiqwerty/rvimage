import os
from pathlib import Path
import tomllib


def main():
    with open(Path(__file__).parent / "pyproject.toml", "rb") as f:
        pyprj = tomllib.load(f)
    version = pyprj["project"]["version"]
    print(f"{version}")
    os.system(f"git tag -a pypi-pub-v{version}")
    os.system("uv build")
    os.system("uv publish")


if __name__ == "__main__":
    main()
