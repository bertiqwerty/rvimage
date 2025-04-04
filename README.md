[![Crate](https://img.shields.io/crates/v/rvimage.svg)](https://crates.io/crates/rvimage)
[![CI workflow](https://github.com/bertiqwerty/rvimage/actions/workflows/rust.yml/badge.svg)](https://github.com/bertiqwerty/rvimage)
[![dependency status](https://deps.rs/repo/github/bertiqwerty/rvimage/status.svg)](https://deps.rs/repo/github/bertiqwerty/rvimage)
[![crates io downloads](https://img.shields.io/crates/d/rvimage.svg)](https://crates.io/crates/rvimage)


# RV Image

<img src="https://github.com/bertiqwerty/rvimage/blob/main/rvimage/resources/rvimage-logo.png?raw=true" width="64">

RV Image is a **r**emote **v**iewer for **image**s written in Rust. You can view images, e.g., on remote SSH servers or Azure blob storages. Further, RV Image comes with a bounding box, polygon, and brush labeling tool supporting import and export of the [Coco-format](https://cocodataset.org/#home). So far tested on Windows 11 and Mac OS. RV Image is mainly based on [`egui`](https://crates.io/crates/egui) and [`image`](https://crates.io/crates/image).

![rvimage_screen](https://github.com/bertiqwerty/rvimage/blob/main/screenshot.png?raw=true)


## Installation

### Windows with Scoop

[Scoop](https://scoop.sh/) is a command-line installer for Windows. If you have Scoop installed you can run 

```
scoop bucket add rvimage https://github.com/bertiqwerty/rvimage-scoop-bucket
scoop install rvimage
```
and start RV Image with the command
```
rvimage
```

#### Update RV Image

With
```
scoop update
scoop update rvimage
```
you get the latest released version.

#### Install Scoop

Scoop can be installed with Powershell 5.1 or later via
```
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression
```
from the prompt `PS C:\>`. Scoop does not need administration rights. See the [Scoop website](https://scoop.sh/) for more details. 

### Pre-built binaries for Windows and Mac

We have a few pre-built binaries for Windows and MacOS on the [releases page](https://github.com/bertiqwerty/rvimage/releases).

### Cargo

With [Rust installed](https://www.rust-lang.org/tools/install)
you can also
```
cargo install rvimage
```
to install the latest stable release. Additionally to the Rust toolchain, you need a c-compiler, make, and perl in your path, since we use the
[ssh2-crate](https://crates.io/crates/ssh2) with the `vendored-openssl` feature, see [here for more info](https://docs.rs/openssl/latest/openssl/index.html#vendored). 


## Connect to remote

RV Image connects to 

* **SSH/SCP** using the [`ssh2` crate](https://crates.io/crates/ssh2), 
* **local folders** that might be mounts of remote storage, 
* **http-servers** spawned via `python -m http.server`, and
* **Azure blob storages***. 

Example configuration for the connection types can be found below. Images are cached locally in a temporary directory. 

## Optional http navigation server 

When RV Image is started, also an http server is launched as aditional navigation option besides the graphical user interface. The default address is `127.0.0.1:5432`. If occupied, the port will be increased. When sending a
get-request to `/file_label` the image `file_label` is loaded. For this to work, `file_label` must
be in the currently opened folder. 

## Configuration

To configure RV Image open `Settings` from the main menu. Many options can only be adapted after clicking
on `Open in Editor`. The configuration is separated into user-specific and project-specific options.
The project specific options are persisted in the project file. The user-specific options are persisted
in `%USERPROFILE%/.rvimage/rv_cfg_usr.toml` or `$HOME/.rvimage/rv_cfg_usr.toml` depending on your operating system.
For SSH currently, only authorization with key-files without passphrase is supported.
```
[usr]
n_autosave = 2
current_prj_path = "prjpath.json"
# "NoCache" for not caching at all or "FileCache" for caching files
# in a configurable folder.
cache = "FileCache"  
# how long should an image be shown before switching to the next in case
# page-up or -down is held
image_change_delay_on_held_key_ms = 300

# Address of the http control server, default is 127.0.0.1:5432
# http_address = address:port

# If you do not want to use the temporary directory of your OS,
# you can add something else.
# tmpdir = 

[usr.file_cache_args]
n_prev_images = 2  # number of images to be cached previous to the selected one
n_next_images = 8  # number of images to be cached following the selected one
n_threads = 4  # number of threads to be used for background file caching
clear_on_close = true  # clear the cache when RV Image is closed
cachedir = "somefolder"  # folder where cached files are stored

[usr.ssh]
user = "your username"
ssh_identity_file_path = "somepath/.ssh/id_file_with_private_key"

[prj]
# We support the connections "Local", "Ssh", "PyHttp", or "AzureBlob"
connection = "Ssh"
[prj.ssh]             
# Local folders can interactively be chosen via file dialog.
# Remote folders are restricted to one of the following list. 
remote_folder_paths = [
    "folder on your server", 
    "another folder"
]
address = "address:port"  # port is usually 22

[prj.py_http_reader_cfg]
# The server is expected to be started via `python -m http.server` in 
# some folder. The content of this folder is then accessible.  
server_addresses = ['http://localhost:8000/']

[prj.azure_blob]
# With a connection string you can view the images inside a blob storage.
# The connection_string_path should point to file that contains just the 
# connection string or a line with 
# `CONNECTION_STRING = ` or `AZURE_CONNECTION_STRING = `.
connection_string_path = "connection_str.txt"
container_name = "images"
# The prefix is also called folder in the Microsoft Azure Storage Explorer.
# Currently, you cannot choose this interactively.
prefix = ""

```

## Labeling Tools

RV Image comes with two labeling tools:

1. Draw bounding boxes and polygons and export in the [Coco format](https://cocodataset.org/#format-data).
2. Draw brush lines and export as [Coco-file](https://cocodataset#format-data) with run-length-encodings. Thereby, we ignore the `iscrowd=true` convention that usually comes with run-length-encoded annotations in Coco-files.

All annotations are also stored in the project file in json format.

Besides labeling tools we also provide means to filter the images to select from and different ways to zoom in and out.

### Zoom

You can zoom anytime with holding <kbd>Ctrl</kbd> and pressing <kbd>+</kbd> or <kbd>-</kbd>. Additionally, you can operate the mouse wheel while holding <kbd>Ctrl</kbd>.

You can also use the separate zoom tool and draw a box on the image area you want to see enlarged. 

To move the zoomed area hold <kbd>Ctrl</kbd> and drag
with the left mouse button. 

### Filter Expressions for Image Files

You can filter for image files to appear in the left selection area. The entered string will reveal those
images that contain the string in their pathname. 

Besides based on the pathname, you can also filter based on the labels and attributes you have used:

1. `nolabel` reveals all images that have not been labeled with the currently active tool.
2. `anylabel` reveals all images that have been labeled with the currently active tool.
3. `label(<label-name>)` reveals all images that have a label of the class `<label-name>` for the currently active tool. 
   For instance, if the bounding box tool is active `label(foreground)` will reveal all images that contain bounding boxes
   or polygons of the class `foreground`. Some special characters in the label names
   might lead to troubles.
4. `tool(<tool-name>)` reveals all images that have been labeled with the tool `<tool-name>`. 
   For instance, `tool(Brush)` will reveal all images that have been labeled with the brush tool.
5. `attr(<attr-name>:<attr-val>)` reveals all images that have the attribute `<attr-name>` set to `<attr-val>`.
6. `attr(<attr-name>:<attr-val-min>-<attr-val-max)` reveals all images that have the attribute `<attr-name>` set between `<attr-val-min>` and `<attr-val-max>`.

Filter strings can be combined with `&&`, `||`, and `!`. For instance
- `!nolabel` corresponds to `anylabel`
- `1055.png || label(cat)` reveals all images that either have a `1055.png` as part of their full pathname or contain 
  the label `cat` from the currently active labeling tool. 

### Bounding Boxes and Polygons

RV Image comes with a simple bounding box and polygon labeling tool that can export to and import from the [Coco format](https://cocodataset.org/#format-data).
For an import to work, the folder that contains the images needs to be opened beforehand. To filter for files that contain bounding boxes of a specific label, one can put `label(<name-of-label>)` into the filter text field. Thereby, `<name-of-label>` needs to be replaced by the real name of the label. To filter for unlabeled files use `nolabel`. Filters including filename-strings can be combined with `&&`, `||`, and `!`.

There two main ways to draw a polygon or a box:
1. One corner per left-click for a polygon. Finish by right-click. One left-click and one right-clicks for a box.
2. Drag the left mouse button. A polygon will follow you mouse.

| event                                                                                | action                                                                                   |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| drag and release left mouse                                                          | draw polygon                                                                             |
| first left click                                                                     | start drawing mode                                                                       |
| $n$-th left click with $n>1$                                                         | add polygon vertex                                                                       |
| right click                                                                          | finish drawing box or polygon                                                            |
| <kbd>Alt</kbd> + left click during box/polygon drawing                               | delete last vertex added                                                                 |
| left click on corner of box                                                          | start drawing mode and move vertex                                                       |
| <kbd>Ctrl</kbd> + left click on box                                                  | select box                                                                               |
| <kbd>Alt</kbd> + left click on box                                                   | select box and deselect others and switch to currently selected label                    |
| hold right button                                                                    | move selected boxes                                                                      |
| <kbd>Shift</kbd> + left click on box                                                 | select all boxes with overlap with the maximal span of this box and other selected boxes |
| <kbd>Ctrl</kbd> + <kbd>A</kbd>                                                       | select all boxes                                                                         |
| <kbd>Delete</kbd>                                                                    | remove selected boxes                                                                    |
| <kbd>Ctrl</kbd> + <kbd>D</kbd>                                                       | deselect all boxes                                                                       |
| <kbd>Ctrl</kbd> + <kbd>H</kbd>                                                       | hide all boxes                                                                           |
| <kbd>C</kbd>                                                                         | clone selected boxes at mouse position and move selection to new box                     |
| <kbd>Ctrl</kbd> + <kbd>C</kbd>                                                       | copy all selected boxes to clipboard                                                     |
| <kbd>Ctrl</kbd> + <kbd>V</kbd>                                                       | paste boxes without existing duplicate from clipboard                                    |
| <kbd>V</kbd>                                                                         | activate auto-paste on image change                                                      |
| <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd>                   | move bottom right corner of all selected boxes                                           |
| <kbd>Ctrl</kbd> + <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd> | move top left corner of all selected boxes                                               |
| <kbd>Alt</kbd> + <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd>  | move all selected boxes                                                                  |
| change label                                                                         | labels of selected boxes/polygons are changed                                            |
| <kbd>L</kbd> | Toggle label display between none, index-sorted-left-right, index-sorted-top-bottom, and category                                            |


### Brush Tool

| event                                                                              | action                                                          |
| ---------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| left click                                                                         | draw circle if not in erase mode, else erase close brush stroke |
| hold left mouse                                                                    | draw brush if not in erase mode                                 |
| <kbd>E</kbd>                                                                       | activate erase mode                                             |
| <kbd>Ctrl</kbd> + click with left mouse                                            | select brush                                                    |
| <kbd>Ctrl</kbd> + <kbd>C</kbd>/<kbd>V</kbd>/<kbd>A</kbd>/<kbd>H</kbd>/<kbd>D</kbd>/<kbd>L</kbd> | see bounding box tool                                           |
| <kbd>Delete</kbd>                                                                  | delete selected strokes                                         |
| change label                                                                       | labels of selected strokes are changed                          |
| <kbd>T</kbd>/<kbd>I</kbd>                                                          | increase thickness/intensity                                    |
| <kbd>Alt</kbd> + <kbd>T</kbd>/<kbd>I</kbd>                                         | decrease thickness/intensity                                    |


---
\* <sub>The connection to Azure blob storages has `tokio`, `futures`, `azure_storage`, and `azure_storage_blob` as additional dependencies, since the used [Azure SDK](https://github.com/Azure/azure-sdk-for-rust) is implemented `async`hronously and needs `tokio`. However, the rest of RV Image uses its own small threadpool implementation. Hence, the Azure blob storage connection is implemented as Cargo-feature `azure_blob` that is enabled by default.</sub>
