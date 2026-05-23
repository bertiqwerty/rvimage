use std::path::Path;
use std::time::Duration;

use image::codecs::png::PngEncoder;
use image::{self, DynamicImage, ExtendedColorType, ImageEncoder};
use reqwest::blocking::multipart;

use rvimage_domain::{BbF, Canvas, GeoFig, RvResult, rverr, to_rv};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::parameters::ParamMap;
use crate::rest_data::RestData;
use crate::wand_util::serialize_or_default;
use crate::{InstanceAnnotate, file_util};
use crate::{tools_data::LabelInfo, tools_data::annotations::InstanceAnnotations};

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
        zoom_box: Option<BbF>,
    ) -> RvResult<WandAnnotationsOutput>;
}

pub struct RestWand {
    data: RestData,
}

impl RestWand {
    pub fn new(url: String, authorization: Option<&str>, timeout_ms: usize) -> Self {
        Self {
            data: RestData::new(url, authorization, timeout_ms, "predict"),
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
        zoom_box: Option<BbF>,
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
        let param_json_str = serialize_or_default(parameters)?;
        let zoom_box_json_str = serde_json::to_string(&zoom_box).map_err(to_rv)?;
        let form = multipart::Form::new()
            .part(
                "image",
                multipart::Part::bytes(image_bytes).file_name(filename),
            )
            .part("parameters", multipart::Part::text(param_json_str))
            .part("input_annotations", multipart::Part::text(annos_json_str))
            .part("zoom_box", multipart::Part::text(zoom_box_json_str));
        let url = format!("{}?active_tool={active_tool}", self.data.url);

        tracing::info!("Sending predictive labeling request to {url}");
        let response = self
            .data
            .client
            .post(&url)
            .headers(self.data.headers.clone())
            .multipart(form)
            .timeout(Duration::from_millis(self.data.timeout_ms as u64))
            .send()
            .map_err(to_rv)?;
        if response.status().is_success() {
            let segs = response.json::<WandAnnotationsOutput>().map_err(to_rv)?;
            Ok(segs)
        } else {
            let status = response.status();
            let err_msg = response
                .text()
                .unwrap_or("no error message available".into());
            Err(rverr!(
                "predictive labelling failed with status {} and error message '{}'",
                status,
                err_msg
            ))
        }
    }
}

#[cfg(test)]
use crate::{defer, parameters::ParamVal, tools::BBOX_NAME, tracing_setup::init_tracing_for_tests};
#[cfg(test)]
use rvimage_domain::BbI;
#[cfg(test)]
use std::{
    process::{Command, Stdio},
    thread,
};

#[test]
fn test() {
    init_tracing_for_tests();
    let manifestdir = env!("CARGO_MANIFEST_DIR").replace("\\", "/");
    let mut child = if cfg!(target_os = "windows") {
        let script_addr = format!("{manifestdir}/resources/test_data/scripts/start_restserver.bat");
        Command::new(script_addr)
            .arg(&manifestdir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to start FastAPI server")
    } else {
        let script = format!(
            r#"
                export PYTHONPATH=../rvimage-py
                cd {manifestdir}/../rest-testserver
                uv run --no-cache fastapi run run.py&
            "#
        );

        Command::new("bash")
            .arg("-c")
            .arg(script)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to start FastAPI server")
    };
    defer!(|| child.kill().expect("Failed to kill the server"));

    tracing::debug!("FastAPI server started");
    thread::sleep(Duration::from_secs(5));
    fn test_inner(url: &str, manifestdir: &str) {
        let w = RestWand::new(url.into(), None, 60000);
        let p = format!("{manifestdir}/resources/rvimage-logo.png");
        let mut m = ParamMap::new();
        m.insert("some_param".into(), ParamVal::Float(Some(1.0)));

        let im = image::open(&p).unwrap();
        let bbox_annos = InstanceAnnotations::from_elts_cats(
            vec![GeoFig::BB(BbF::from_arr(&[0.0, 0.0, 5.0, 5.0]))],
            vec![1],
        );
        let c = Canvas::from_box(BbI::from_arr(&[11, 11, 5, 5]), 1.0);
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
                None,
                annos.clone(),
                Some(BbF::from_arr(&[0.0, 0.0, 1.5, 1.5])),
            )
            .unwrap();
        let WandAnnotationsOutput {
            bbox: ret_bbox_data,
            brush: ret_brush_data,
        } = seg;
        let ret_bbox_data = ret_bbox_data.unwrap();
        let ret_brush_data = ret_brush_data.unwrap();
        macro_rules! assert_sendback {
            ($tool:ident, $ret:expr) => {
                for (a, cat_idx, is_selected) in annos.$tool.as_ref().unwrap().annos.iter() {
                    let mut found = false;
                    for (r_a, r_cat_idx, r_is_selected) in $ret.iter() {
                        if a == r_a && is_selected == r_is_selected && cat_idx == r_cat_idx {
                            found = true;
                        }
                    }
                    assert!(found);
                }
            };
        }
        assert_sendback!(bbox, ret_bbox_data);
        assert_sendback!(brush, ret_brush_data);
        assert_eq!(
            ret_bbox_data.elts()[0].enclosing_bb(),
            BbF::from_arr(&[21.0, 31.0, 9.0, 9.0])
        );
        assert_eq!(vec![1, 1, 1], ret_brush_data.elts()[0].mask);
        assert_eq!(
            Canvas::from_box(BbI::from_arr(&[23, 30, 3, 1]), 1.0),
            ret_brush_data.elts()[0]
        );
        assert_eq!(
            Canvas::from_box(BbI::from_arr(&[5, 76, 1, 4]), 1.0),
            ret_brush_data.elts()[1]
        );
    }
    test_inner("http://127.0.0.1:8000/", &manifestdir);
    test_inner("http://127.0.0.1:8000", &manifestdir);
    test_inner("http://127.0.0.1:8000/predict", &manifestdir);
    test_inner("http://127.0.0.1:8000/predict/", &manifestdir);
}
