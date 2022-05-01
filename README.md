# RV Image
**R**emote **v**iewer for **image**s in Rust that connects via 

* SSH/SCP relying on the tools being accessible via command line or
* local folders that might be mounts of remote storage. 

That is, to be able to connect to an ssh server, the commands `ssh` and `scp` need to be accessible from the terminal. Images are cached locally in a temporary directory. So far only tested on Windows 10. Currently, only RGB images with 8 bits per pixel and channel are supported. They have to be either `.png` or `.jpg`. RV Image is mainly based on [Egui](https://crates.io/crates/egui), [Image](https://crates.io/crates/image), and [Pixels](https://crates.io/crates/pixels).


## Configuration
Create a file `rv_cfg.toml` in the folder from where you execute RV Image with the following content. Currently, only authorization with key-files without password is supported. The config will be read on start up of the application. To activate changes you have to restart.
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
# After the application started you can change the remote folder. For local folders, this is possible. 
remote_folder_path = "folder on your server"
address = "address of your server"
user = "your username"
ssh_identity_file_path = "somepath/.ssh/id_file_with_private_key"
# Your scp command.
# If not given, we use 
# ["cmd", "/C", "scp"]
# on Windows and 
# ["sh", "-c", "scp"]
# otherwise (untested).
# scp_command = 

```
