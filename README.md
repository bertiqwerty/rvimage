[![CI workflow](https://github.com/bertiqwerty/rvimage/actions/workflows/rust.yml/badge.svg)](https://github.com/bertiqwerty/rvimage)
# RV Image
RV Image is a **R**emote **v**iewer for **image**s written in Rust. You can view images, e.g., on remote SSH servers or Azure blob storages. Further, RV Image comes with a simple bounding box labeling tool. So far only tested on Windows 10. RV Image is mainly based on [`egui`](https://crates.io/crates/egui), [`image`](https://crates.io/crates/image), and [`pixels`](https://crates.io/crates/pixels).


![grafik](https://user-images.githubusercontent.com/50267830/168975976-45ab6567-ef5e-41a8-8027-2ff210dfda3c.png)


## Connect to remote

RV Image connects to 

* **SSH/SCP** using the [`ssh2` crate](https://crates.io/crates/ssh2), 
* **local folders** that might be mounts of remote storage, 
* **http-servers** spawned via `python -m http.server`, and
* **Azure blob storages***. 

Example configuration for the connection types can be found below. Images are cached locally in a temporary directory. 

## Optional http server to control image selection 

When RV Image is started, also an http server is launched as aditional navigation option besides the graphical user interface. The default address is `127.0.0.1:5432`. If occupied, the port will be increased. When sending a
get-request to `/file_label` the image `file_label` is loaded. For this to work, `file_label` must
be in the currently opened folder. 

## Configuration

Create a file `rv_cfg.toml` in `%USERPROFILE%/.rvimage/rv_cfg.toml` (or probably `$HOME/.rvimage/rv_cfg.toml` under Linux, untested) with the following content. For SSH currently, only authorization with key-files without passphrase is supported.
```
 # We support the connections "Local", "Ssh", "PyHttp", or "AzureBlob"
connection = "Ssh"

# "NoCache" for not caching at all or "FileCache" for caching files in a temp dir.
cache = "FileCache"  

# Address of the http control server, default is 127.0.0.1:5432
# http_address = address:port

# If you do not want to use the temporary directory of your OS, you can add something else.
# tmpdir = 

[file_cache_args]
n_prev_images = 2  # number of images to be cached previous to the selected one
n_next_images = 8  # number of images to be cached following the selected one
n_threads = 4  # number of threads to be used for background file caching

[ssh_cfg]             
# Local folders can interactively be chosen via file dialog. Remote folders are restricted to one of the following list. 
remote_folder_paths = [
    "folder on your server", 
    "another folder"
]
address = "address:port"  # port is usually 22
user = "your username"
ssh_identity_file_path = "somepath/.ssh/id_file_with_private_key"

[py_http_reader_cfg]
# The server is expected to be started via `python -m http.server` in some folder.
# The content of this folder is than accessible.  
server_address = 'http://localhost:8000/'

[azure_blob_cfg]
# With a connection string you can view the images inside a blob storage
connection_string = ''
container_name = ''
# The prefix is also called folder in the Microsoft Azure Storage Explorer.
# Currently, you cannot choose this interactively.
prefix = ''


```

## Bounding Box Labeling Tool

RV Image comes with a simple bounding box labeling that can export to and import from the [Coco format](https://cocodataset.org/#format-data).
For an import to work, the folder that contains the images needs to be opened beforehand. To filter for files that contain bounding boxes of a specific label, one can put `label:<name-of-label>` into the filter text field. Thereby, `<name-of-label>` needs to be replaced by the real name of the label. To filter for unlabeled files use `unlabeled:`.

| event                                                                                | action                                                                                   |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| first left click                                                                     | start drawing box                                                                        |
| second left click                                                                    | finish drawing box                                                                       |
| left click on corner of box                                                          | move corner of box                                                                       |
| hold right button                                                                    | move selected boxes                                                                      |
| <kbd>Ctrl</kbd> + left click on box                                                  | select box                                                                               |
| <kbd>Alt</kbd> + left click on box                                                   | select box and deselect others and switch to currently selected label                    |
| <kbd>Shift</kbd> + left click on box                                                 | select all boxes with overlap with the maximal span of this box and other selected boxes |
| <kbd>Ctrl</kbd> + <kbd>A</kbd>                                                       | select all boxes                                                                         |
| <kbd>Delete</kbd>                                                                    | remove selected boxes                                                                    |
| <kbd>Ctrl</kbd> + <kbd>D</kbd>                                                       | deselect all boxes                                                                       |
| <kbd>C</kbd>                                                                         | clone selected boxes at mouse position and move selection to new box                     |
| <kbd>Ctrl</kbd> + <kbd>C</kbd>                                                       | copy all selected boxes to clipboard                                                     |
| <kbd>Ctrl</kbd> + <kbd>V</kbd>                                                       | paste boxes without existing duplicate from clipboard                                    |
| <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd>                   | move bottom right corner of all selected boxes                                           |
| <kbd>Ctrl</kbd> + <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd> | move top left corner of all selected boxes                                               |
| <kbd>Alt</kbd> + <kbd>Left⬅</kbd>/<kbd>Right➡</kbd>/<kbd>Up⬆</kbd>/<kbd>Down⬇</kbd>  | move all selected boxes                                                                  |
| <kbd>Ctrl</kbd> + <kbd>L</kbd>                                                       | toggle category names                                                                    |

---
\* <sub>The connection to Azure blob storages has `tokio`, `futures`, `azure_storage`, and `azure_storage_blob` as additional dependencies, since the used [Azure SDK](https://github.com/Azure/azure-sdk-for-rust) is implemented `async`hronously and needs `tokio`. However, the rest of RV Image uses its own small threadpool implementation. Hence, the Azure blob storage connection is implemented as Cargo-feature `azure_blob` that is enabled by default.</sub>
