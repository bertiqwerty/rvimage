use crate::{
    cfg::{get_cfg, Connection},
    reader::{local_reader::LocalReader, scp_reader::ScpReader, ReadImageFiles},
    result::RvResult
};
macro_rules! make_reader_from_config {
    ($($names:ident),+) => {
        pub enum ReaderFromConfig {
            $($names($names),)+
        }

        impl ReadImageFiles for ReaderFromConfig {
            fn new() -> Self {
                let cfg = get_cfg().unwrap();
                (match cfg.connection {
                    $(Connection::$names => ReaderFromConfig::$names($names::new()),)+
                })
            }
            fn next(&mut self) {
                match self {
                    $(Self::$names(x) => x.next(),)+
                }
            }
            fn prev(&mut self) {
                match self {
                    $(Self::$names(x) => x.prev(),)+
                }
            }
            fn read_image(
                &mut self,
                file_selected_idx: usize,
            ) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
                match self {
                    $(Self::$names(x) => x.read_image(file_selected_idx),)+
                }
            }
            fn open_folder(&mut self) -> RvResult<()> {
                match self {
                    $(Self::$names(x) => x.open_folder(),)+
                }
            }
            fn file_selected_idx(&self) -> Option<usize> {
                match self {
                    $(Self::$names(x) => x.file_selected_idx(),)+
                }
            }
            fn selected_file(&mut self, idx: usize) {
                match self {
                    $(Self::$names(x) => x.selected_file(idx),)+
                };
            }
            fn list_file_labels(&self) -> RvResult<Vec<String>> {
                match self {
                    $(Self::$names(x) => x.list_file_labels(),)+
                }
            }
            fn folder_label(&self) -> RvResult<String> {
                 match self {
                    $(Self::$names(x) => x.folder_label(),)+
                }
            }
            fn file_selected_label(&self) -> RvResult<String> {
                  match self {
                    $(Self::$names(x) => x.file_selected_label(),)+
                }
            }
        }
    };
}
make_reader_from_config!(LocalReader, ScpReader);
