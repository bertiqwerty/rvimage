use std::collections::HashMap;

use reqwest::blocking::multipart;
use rvimage_domain::{Canvas, GeoFig, RvResult, to_rv};
use serde::{Deserialize, Serialize};

use crate::{
    InstanceAnnotate, ToolsDataMap,
    parameters::ParamMap,
    rest_data::RestData,
    tools::{BBOX_NAME, BRUSH_NAME},
    tools_data::{AccessInstanceData, LabelInfo, annotations::InstanceAnnotations},
    wand_util::serialize_or_default,
};

#[derive(Serialize, Clone)]
pub struct AnnosWithInfo<'a, T>
where
    T: InstanceAnnotate,
{
    pub annos: HashMap<&'a str, &'a InstanceAnnotations<T>>,
    pub labelinfo: &'a LabelInfo,
}
#[derive(Serialize, Clone)]
pub struct WandPrjAnnotationsInput<'a> {
    pub bbox: Option<AnnosWithInfo<'a, GeoFig>>,
    pub brush: Option<AnnosWithInfo<'a, Canvas>>,
}

impl<'a> WandPrjAnnotationsInput<'a> {
    pub fn from_tdm(tools_data_map: &'a ToolsDataMap) -> Self {
        let bbox = tools_data_map
            .get_specifics(BBOX_NAME)
            .and_then(|s| {
                s.bbox()
                    .map(|bb| (bb.annotations_map(), bb.label_info()))
                    .ok()
            })
            .map(|(am, li)| {
                (
                    am.iter()
                        .map(|(k, (v, _))| (k.as_str(), v))
                        .collect::<HashMap<&str, &InstanceAnnotations<GeoFig>>>(),
                    li,
                )
            })
            .map(|(annos, labelinfo)| AnnosWithInfo { annos, labelinfo });

        let brush = tools_data_map
            .get_specifics(BRUSH_NAME)
            .and_then(|s| {
                s.brush()
                    .map(|br| (br.annotations_map(), br.label_info()))
                    .ok()
            })
            .map(|(am, li)| {
                (
                    am.iter()
                        .map(|(k, (v, _))| (k.as_str(), v))
                        .collect::<HashMap<&str, &InstanceAnnotations<Canvas>>>(),
                    li,
                )
            })
            .map(|(annos, labelinfo)| AnnosWithInfo { annos, labelinfo });
        Self { bbox, brush }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct WandPrjAnnotationsOutput {
    pub bbox: Option<HashMap<String, InstanceAnnotations<GeoFig>>>,
    pub brush: Option<HashMap<String, InstanceAnnotations<Canvas>>>,
}

pub trait WandPrjAnnotator {
    /// Predictions for the whole project
    ///
    /// # Arguments
    ///
    /// * annotations_input: all annotations for all images
    /// * parameters: parameters that can be defined in the UI and might be
    ///   necessary for the predictor
    ///
    fn predict<'a>(
        &self,
        annotations_input: WandPrjAnnotationsInput<'a>,
        parameters: Option<&ParamMap>,
    ) -> RvResult<WandPrjAnnotationsOutput>;
}

pub struct RestWandPrjAnnotator {
    data: RestData,
}
impl RestWandPrjAnnotator {
    pub fn new(url: String, authorization: Option<&str>, timeout_ms: usize) -> Self {
        Self {
            data: RestData::new(url, authorization, timeout_ms, "predict"),
        }
    }
}

impl WandPrjAnnotator for RestWandPrjAnnotator {
    fn predict<'a>(
        &self,
        annos_input: WandPrjAnnotationsInput<'a>,
        parameters: Option<&ParamMap>,
    ) -> RvResult<WandPrjAnnotationsOutput> {
        let annos_json_str = serde_json::to_string(&annos_input).map_err(to_rv)?;
        let param_json_str = serialize_or_default(parameters)?;
        let form = multipart::Form::new()
            .part("parameters", multipart::Part::text(param_json_str))
            .part("input_annotations", multipart::Part::text(annos_json_str));
        Ok(WandPrjAnnotationsOutput::default())
    }
}

pub fn submit(tools_data_map: &ToolsDataMap) {}
