use std::path::Path;

use image::{self, DynamicImage};
use reqwest::blocking::multipart;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use rvimage_domain::{rverr, to_rv, RvResult};

use crate::result::trace_ok_err;
use crate::{file_util, tools_data::coco_io::CocoExportData};

#[allow(dead_code)]
pub enum ImageForPrediction<'a> {
    Image(&'a DynamicImage),
    ImagePath(&'a Path),
}

#[allow(dead_code)]
pub trait Wand {
    fn predict(
        &self,
        im: ImageForPrediction,
        annotations: Option<&CocoExportData>,
    ) -> RvResult<CocoExportData>;
}

pub struct RestWand {
    url: String,
    headers: HeaderMap,
    client: reqwest::blocking::Client,
}

#[allow(dead_code)]
impl RestWand {
    pub fn new(url: String, authorization: Option<&str>) -> Self {
        let client = reqwest::blocking::Client::new();
        let mut headers = HeaderMap::new();
        if let Some(s) = authorization {
            if let Some(s) = trace_ok_err(HeaderValue::from_str(s)) {
                headers.insert(AUTHORIZATION, s);
            }
        }

        Self {
            url,
            headers,
            client,
        }
    }
}

impl Wand for RestWand {
    fn predict(
        &self,
        im: ImageForPrediction,
        annotations: Option<&CocoExportData>,
    ) -> RvResult<CocoExportData> {
        match im {
            ImageForPrediction::Image(_) => {
                 Err(rverr!("RestWand needs a filepath, not the image itself, image prediction not implemented yet"))
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
                        multipart::Part::text(
                            if let Some(annos) = annotations {
                                serde_json::to_string(annos)
                            } else {
                                serde_json::to_string(&CocoExportData::default())
                            }
                            .map_err(to_rv)?,
                        ),
                    );

                self.client
                    .post(&self.url)
                    .headers(self.headers.clone())
                    .multipart(form)
                    .send()
                    .map_err(to_rv)?
                    .json::<CocoExportData>()
                    .map_err(to_rv)
            }
        }
    }
}

#[cfg(test)]
use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn test() {
    let script = format!(
        r#"
        cd {}/../rest-testserver
        uv sync
        uv run fastapi run run.py&
    "#,
        env!("CARGO_MANIFEST_DIR")
    );
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start FastAPI server");
    thread::sleep(Duration::from_secs(2));
    let w = RestWand::new("http://localhost:8000/predict".into(), None);
    let p = "/Users/b/prj/rvimage/rvimage/resources/rvimage-logo.png";
    let data = CocoExportData::default();
    println!("{data:?}");
    println!("{:#?}", serde_json::to_string(&data));
    w.predict(ImageForPrediction::ImagePath(Path::new(p)), None)
        .unwrap();
    child.kill().expect("Failed to kill the server");
}
