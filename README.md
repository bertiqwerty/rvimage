# RV Image
**R**emote **v**iewer for **image**s in Rust that connects via 

* SSH/SCP relying on the tools being accessible via command line or
* local folders that might be mounts of remote storage. 

Images are cached locally in a temporary directory. So far only tested on Windows 10. Currently, only RGB images with 8 bits per pixel and channel are supported.
