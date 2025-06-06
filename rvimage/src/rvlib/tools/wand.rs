use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{self, DynamicImage, ExtendedColorType, ImageEncoder};
use reqwest::blocking::multipart;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use rvimage_domain::{to_rv, RvResult};
use std::io::Cursor;

use crate::cfg::ExportPath;
use crate::result::trace_ok_err;
use crate::tools_data::parameters::ParamMap;
use crate::tools_data::{BboxToolData, BrushToolData};
use crate::Rot90ToolData;
use crate::{file_util, tools_data::coco_io::CocoExportData};

#[allow(dead_code)]
pub struct ImageForPrediction<'a> {
    pub image: &'a DynamicImage,
    pub path: Option<&'a Path>,
}

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
    fn predict<'a>(
        &self,
        im: ImageForPrediction,
        label_name: impl Iterator<Item = &'a str>,
        parameters: Option<&ParamMap>,
        bbox_data: Option<&BboxToolData>,
        brush_data: Option<&BrushToolData>,
    ) -> RvResult<(BboxToolData, BrushToolData)>;
}

pub struct RestWand<'a> {
    url: String,
    headers: HeaderMap,
    client: reqwest::blocking::Client,
    rotation_data: Option<&'a Rot90ToolData>,
}

#[allow(dead_code)]
impl<'a> RestWand<'a> {
    pub fn new(
        url: String,
        authorization: Option<&str>,
        rotation_data: Option<&'a Rot90ToolData>,
    ) -> Self {
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
            rotation_data,
        }
    }
}

impl<'b> Wand for RestWand<'b> {
    fn predict<'a>(
        &self,
        im: ImageForPrediction,
        label_names: impl Iterator<Item = &'a str>,
        parameters: Option<&ParamMap>,
        bbox_data: Option<&BboxToolData>,
        brush_data: Option<&BrushToolData>,
    ) -> RvResult<(BboxToolData, BrushToolData)> {
        let rgb_image = im.image.to_rgb8();
        let (width, height) = rgb_image.dimensions();

        let mut image_bytes = Vec::new();
        let cursor = Cursor::new(&mut image_bytes);

        let encoder = PngEncoder::new(cursor);
        encoder
            .write_image(&rgb_image, width, height, ExtendedColorType::Rgb8)
            .map_err(to_rv)?;

        let filename = if let Some(p) = im.path {
            file_util::to_name_str(p)?.to_string()
        } else {
            "tmpfile.png".into()
        };
        let mut bbox_exp = if let Some(bbox_data) = bbox_data {
            CocoExportData::from_tools_data(bbox_data.clone(), self.rotation_data, None)
        } else {
            CocoExportData::default()
        };
        let mut brush_exp = if let Some(brush_data) = brush_data {
            CocoExportData::from_tools_data(brush_data.clone(), self.rotation_data, None)
        } else {
            CocoExportData::default()
        };
        bbox_exp.append(&mut brush_exp);
        let anno_json_str = serde_json::to_string(&brush_exp).map_err(to_rv)?;
        let param_json_str = if let Some(p) = parameters {
            serde_json::to_string(p)
        } else {
            serde_json::to_string(&ParamMap::default())
        }
        .map_err(to_rv)?;
        let form = multipart::Form::new()
            .part(
                "image",
                multipart::Part::bytes(image_bytes).file_name(filename),
            )
            .part("parameters", multipart::Part::text(param_json_str))
            .part("annotations", multipart::Part::text(anno_json_str));
        let paramsquery = label_names
            .map(|n| format!("label_names={n}"))
            .reduce(|l1, l2| format!("{l1}&{l2}"));
        let paramsquery = paramsquery.map(|pq| format!("?{pq}")).unwrap_or_default();
        let url = format!("{}{}", self.url, paramsquery);

        tracing::info!("Sending predictive labeling request to {url}");
        let coco_export_data = self
            .client
            .post(&url)
            .headers(self.headers.clone())
            .multipart(form)
            .send()
            .map_err(to_rv)?
            .json::<CocoExportData>()
            .map_err(to_rv)?;
        if coco_export_data.is_empty() {
            Ok((BboxToolData::default(), BrushToolData::default()))
        } else {
            coco_export_data.convert_to_toolsdata(ExportPath::default(), self.rotation_data)
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
    let w = RestWand::new("http://127.0.0.1:8000/predict".into(), None, None);
    let p = format!("{}/resources/rvimage-logo.png", manifestdir);
    let mut m = ParamMap::new();
    m.insert("some_param".into(), ParamVal::Float(Some(1.0)));

    let im = image::open(&p).unwrap();
    let (bbox_data, brush_data) = w
        .predict(
            ImageForPrediction {
                image: &im,
                path: Some(Path::new(&p)),
            },
            ["some_label"].iter().map(|s| *s),
            Some(&m),
            None,
            None,
        )
        .unwrap();
    tracing::debug!("Coco export data: {bbox_data:?}");
    tracing::debug!("Coco export data: {brush_data:?}");
    child.kill().expect("Failed to kill the server");
}
