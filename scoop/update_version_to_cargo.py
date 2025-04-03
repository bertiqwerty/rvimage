import json
from pathlib import Path
import tomllib
import urllib.request
import click


@click.command()
@click.argument("bucketpath", type=click.Path(exists=True, file_okay=False))
def main(bucketpath: str):
    manifest = Path(f"{bucketpath}/rvimage.json")
    with open(manifest, "r") as f:
        scoop_data = json.load(f)

    with open("rvimage/Cargo.toml", "rb") as f:
        cargo_data = tomllib.load(f)
    cargo_version = cargo_data["package"]["version"]
    old_version = scoop_data["version"]
    new_version = f"v{cargo_version}"
    if old_version == new_version:
        print("No version change")
        return
    scoop_data["version"] = new_version
    url = scoop_data["architecture"]["64bit"]["url"]
    scoop_data["architecture"]["64bit"]["url"] = url.replace(old_version, new_version)
    extract_dir = scoop_data["architecture"]["64bit"]["extract_dir"]
    scoop_data["architecture"]["64bit"]["extract_dir"] = extract_dir.replace(
        old_version, new_version
    )
    checksum_file, _ = urllib.request.urlretrieve(
        "https://github.com/bertiqwerty/rvimage/releases/download/"
        f"{new_version}/rvimage-{new_version}-x86_64-pc-windows-msvc.zip.sha256",
    )
    with open(checksum_file, "r") as f:
        checksum = f.readlines()[1].strip()
    scoop_data["architecture"]["64bit"]["hash"] = checksum
    print(scoop_data)
    with open(manifest, "w") as f:
        json.dump(scoop_data, f, indent=4)


if __name__ == "__main__":
    main()
