use std::path::Path;

use image::{self, DynamicImage};
use reqwest::blocking::multipart;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use rvimage_domain::{rverr, to_rv, RvResult};

use crate::result::trace_ok_err;
use crate::tools_data::parameters::ParamMap;
use crate::{file_util, tools_data::coco_io::CocoExportData};

#[allow(dead_code)]
pub enum ImageForPrediction<'a> {
    Image(&'a DynamicImage),
    ImagePath(&'a Path),
}

#[allow(dead_code)]
pub trait Wand {
    /// Prediction for predictive labelling
    ///
    /// # Arguments
    ///
    /// * im: path to image or loaded image instance, implementations
    ///   of [`Wand`] might return an error if an unsupported option is passed.
    /// * label_names: all the labels we want to have predictions for
    /// * annotations: currently available annotations, optionally to be used by
    ///   implementations of [`Wand`]
    ///
    fn predict(
        &self,
        im: ImageForPrediction,
        label_name: &[&str],
        parameters: Option<&ParamMap>,
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
        label_names: &[&str],
        parameters: Option<&ParamMap>,
        annotations: Option<&CocoExportData>,
    ) -> RvResult<CocoExportData> {
        match im {
            ImageForPrediction::Image(_) => {
                 Err(rverr!("RestWand needs a filepath, not the image itself, image prediction not implemented yet"))
            }
            ImageForPrediction::ImagePath(path) => {
                let image_bytes = std::fs::read(path).map_err(to_rv)?;
                let filename = file_util::to_name_str(path)?.to_string();
                let anno_json_str = if let Some(annos) = annotations {
                    serde_json::to_string(annos)
                } else {
                    serde_json::to_string(&CocoExportData::default())
                }
                .map_err(to_rv)?;
                let param_json_str = if let Some(p) = parameters {
                    serde_json::to_string(p)
                } else {
                    serde_json::to_string(&ParamMap::default())
                }.map_err(to_rv)?;
                let form = multipart::Form::new()
                    .part(
                        "image",
                        multipart::Part::bytes(image_bytes).file_name(filename)
                    )
                    .part("parameters", multipart::Part::text(param_json_str))
                    .part(
                        "annotations",
                        multipart::Part::text(anno_json_str)
                    );
                let paramsquery = label_names
                    .iter()
                    .map(|n| format!("label_names={n}")).reduce(|l1, l2| format!("{l1}&{l2}"));
                let paramsquery = paramsquery.map(|pq| format!("?{pq}")).unwrap_or_default();
                let url = format!("{}{}",self.url, paramsquery);

                self.client
                    .post(&url)
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
use crate::tools_data::parameters::ParamVal;
#[cfg(test)]
use crate::tracing_setup::init_tracing_for_tests;
#[cfg(test)]
use std::{
    process::{Command, Stdio},
    thread,
    time::Duration,
};

#[test]
fn test() {
    init_tracing_for_tests();
    let manifestdir = env!("CARGO_MANIFEST_DIR");
    let script = format!(
        r#"
        cd {}/../rest-testserver
        uv sync
        uv run fastapi run run.py&
    "#,
        manifestdir
    );
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to start FastAPI server");
    thread::sleep(Duration::from_secs(5));
    let w = RestWand::new("http://127.0.0.1:8000/predict".into(), None);
    let p = format!("{}/resources/rvimage-logo.png", manifestdir);
    let mut m = ParamMap::new();
    m.insert("some_param".into(), ParamVal::Float(Some(1.0)));

    w.predict(
        ImageForPrediction::ImagePath(Path::new(&p)),
        &["some_label"],
        Some(&m),
        None,
    )
    .unwrap();
    child.kill().expect("Failed to kill the server");
}
