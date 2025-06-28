use std::path::Path;

use image::codecs::png::PngEncoder;
use image::{self, DynamicImage, ExtendedColorType, ImageEncoder};
use reqwest::blocking::multipart;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use rvimage_domain::{to_rv, Canvas, GeoFig, RvResult};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::result::trace_ok_err;
use crate::tools_data::annotations::InstanceAnnotations;
use crate::tools_data::parameters::ParamMap;
use crate::tools_data::LabelInfo;
use crate::{file_util, InstanceAnnotate};

#[allow(dead_code)]
pub struct ImageForPrediction<'a> {
    pub image: &'a DynamicImage,
    pub path: Option<&'a Path>,
}

#[derive(Serialize, Clone)]
pub struct AnnosWithInfo<'a, T>
where
    T: InstanceAnnotate,
{
    pub annos: &'a InstanceAnnotations<T>,
    pub labelinfo: &'a LabelInfo,
}

#[derive(Serialize, Clone)]
pub struct WandAnnotationsInput<'a> {
    pub bbox: Option<AnnosWithInfo<'a, GeoFig>>,
    pub brush: Option<AnnosWithInfo<'a, Canvas>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WandAnnotationsOutput {
    pub bbox: Option<InstanceAnnotations<GeoFig>>,
    pub brush: Option<InstanceAnnotations<Canvas>>,
}

pub trait Wand {
    /// Prediction for predictive labelling
    ///
    /// # Arguments
    ///
    /// * im: path to image or loaded image instance, implementations
    ///   of [`Wand`] might return an error if an unsupported option is passed.
    /// * label_names_to_predict: all the labels we want to have predictions for
    /// * parameters: parameters that can be defined in the UI and might be
    ///   necessary for the predictor
    /// * bbox_data: names of all labels and instance annotations such that the
    ///   cat-idxs of the annotations yield the corresponding name of the label.
    ///   This can be helpful for comparison with the iterator of label names to predict.
    /// * brush_data: see bbox_data
    ///
    fn predict<'a>(
        &self,
        im: ImageForPrediction,
        active_tool: &'static str,
        parameters: Option<&ParamMap>,
        annotations_input: WandAnnotationsInput<'a>,
    ) -> RvResult<WandAnnotationsOutput>;
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
    fn predict<'a>(
        &self,
        im: ImageForPrediction,
        active_tool: &'static str,
        parameters: Option<&ParamMap>,
        annos_input: WandAnnotationsInput<'a>,
    ) -> RvResult<WandAnnotationsOutput> {
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
        let annos_json_str = serde_json::to_string(&annos_input).map_err(to_rv)?;
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
            .part("input_annotations", multipart::Part::text(annos_json_str));
        let url = format!("{}?active_tool={active_tool}", self.url);

        tracing::info!("Sending predictive labeling request to {url}");
        let segs = self
            .client
            .post(&url)
            .headers(self.headers.clone())
            .multipart(form)
            .send()
            .map_err(to_rv)?
            .json::<WandAnnotationsOutput>()
            .map_err(to_rv)?;
        Ok(segs)
    }
}

#[cfg(test)]
use crate::{
    tools::BBOX_NAME, tools_data::parameters::ParamVal, tracing_setup::init_tracing_for_tests,
};
#[cfg(test)]
use rvimage_domain::{BbF, BbI};
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
        export PYTHONPATH=../rvimage-py
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

    let im = image::open(&p).unwrap();
    let bbox_annos = InstanceAnnotations::from_elts_cats(
        vec![GeoFig::BB(BbF::from_arr(&[0.0, 0.0, 5.0, 5.0]))],
        vec![1],
    );
    let c = Canvas {
        bb: BbI::from_arr(&[0, 0, 10, 10]),
        mask: vec![0; 100],
        intensity: 1.0,
    };
    let brush_annos = InstanceAnnotations::from_elts_cats(vec![c], vec![1]);
    let labelinfo = LabelInfo::default();
    let bbox_dummy = AnnosWithInfo {
        annos: &bbox_annos,
        labelinfo: &labelinfo,
    };
    let brush_dummy = AnnosWithInfo {
        annos: &brush_annos,
        labelinfo: &labelinfo,
    };
    let annos = WandAnnotationsInput {
        bbox: Some(bbox_dummy),
        brush: Some(brush_dummy),
    };
    let seg = w
        .predict(
            ImageForPrediction {
                image: &im,
                path: Some(Path::new(&p)),
            },
            BBOX_NAME,
            Some(&m),
            annos.clone(),
        )
        .unwrap();
    let WandAnnotationsOutput {
        bbox: bbox_data,
        brush: brush_data,
    } = seg;
    assert_eq!(bbox_data, annos.bbox.map(|b| b.annos.clone()));
    assert_eq!(brush_data, annos.brush.map(|b| b.annos.clone()));
    child.kill().expect("Failed to kill the server");
}
