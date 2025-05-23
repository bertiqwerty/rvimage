use std::path::Path;

use image::{self, DynamicImage};
use reqwest::blocking::multipart;
use rvimage_domain::{rverr, to_rv, RvResult};

use crate::{file_util, tools_data::coco_io::CocoExportData};
pub enum ImageForPrediction<'a> {
    Image(&'a DynamicImage),
    ImagePath(&'a Path),
}

pub trait Wand {
    fn predict<'a>(
        &self,
        im: ImageForPrediction<'a>,
        annotations: &CocoExportData,
    ) -> RvResult<CocoExportData>;
}

pub struct RestWand {
    url: String,
    headers: Vec<(String, String)>,
    client: reqwest::blocking::Client,
}

impl RestWand {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::new();
        Self {
            url: String::new(),
            headers: Vec::new(),
            client,
        }
    }
}

impl Wand for RestWand {
    fn predict<'a>(
        &self,
        im: ImageForPrediction<'a>,
        annotations: &CocoExportData,
    ) -> RvResult<CocoExportData> {
        match im {
            ImageForPrediction::Image(_) => {
                return Err(rverr!("RestWand needs a filepath, not the image itself, image prediction not implemented yet"));
            }
            ImageForPrediction::ImagePath(path) => {
                let image_bytes = std::fs::read(path).map_err(to_rv)?;
                let filename = file_util::to_name_str(path)?.to_string();
                let form = multipart::Form::new()
                    .part(
                        "image",
                        multipart::Part::bytes(image_bytes).file_name(filename),
                    )
                    .part(
                        "annotations",
                        multipart::Part::text(serde_json::to_string(annotations).map_err(to_rv)?),
                    );

                self.client
                    .post(&self.url)
                    .multipart(form)
                    .send()
                    .map_err(to_rv)?
                    .json::<CocoExportData>()
                    .map_err(to_rv)
            }
        }
    }
}
