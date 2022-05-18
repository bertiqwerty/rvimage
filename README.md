[![CI workflow](https://github.com/bertiqwerty/rvimage/actions/workflows/rust.yml/badge.svg)](https://github.com/bertiqwerty/rvimage)
# RV Image
**R**emote **v**iewer for **image**s in Rust that connects via 

* SSH/SCP using the [`ssh2` crate](https://crates.io/crates/ssh2) and 
* local folders that might be mounts of remote storage. 

Images are cached locally in a temporary directory. So far only tested on Windows 10. Currently, only RGB images with 8 bits per pixel and channel are supported. They have to be either `.png` or `.jpg`. RV Image is mainly based on [`egui`](https://crates.io/crates/egui), [`image`](https://crates.io/crates/image), and [`pixels`](https://crates.io/crates/pixels).


## Configuration
Create a file `rv_cfg.toml` in `%USERPROFILE%/.rvimage/rv_cfg.toml` (or probably `$HOME/.rvimage/rv_cfg.toml` under Linux, untested) with the following content. Currently, only authorization with key-files without passphrase is supported.
```
connection = "Ssh" # "Local" or "Ssh", Local for local folder
cache = "FileCache"  # "NoCache" for not caching at all or "FileCache" for caching files in a temp dir.
[file_cache_args]
n_prev_images = 2  # number of images to be cached previous to the selected one
n_next_images = 8  # number of images to be cached following the selected one
n_threads = 4  # number of threads to be used for background file caching
# If you do not want to use the temporary directory of your OS, you can add something else.
# tmpdir = 
[ssh_cfg]             
# You cannot change the ssh-remote folders interactively. For local folders, this is possible. 
remote_folder_path = "folder on your server"
address = "address:port"  # port is usually 22
user = "your username"
ssh_identity_file_path = "somepath/.ssh/id_file_with_private_key"
```
