mod data;
pub use data::{WandManyData, WandManyMessage};
use std::path::Path;

use reqwest::blocking::multipart;
use rvimage_domain::{Canvas, GeoFig, RvResult, ShapeI, to_rv};
use serde::{Deserialize, Serialize};

use crate::{
    InstanceAnnotate, ToolsDataMap,
    parameters::{ParamMap, ParamMapUntagged},
    rest_data::RestData,
    result::trace_ok_err,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{
        AccessInstanceData, ExportAsCoco, LabelInfo, LabelMap, annotations::InstanceAnnotations,
    },
    wand_util::serialize_or_default,
};

#[derive(Serialize, Clone)]
pub struct AnnosWithInfo<'a, T>
where
    T: InstanceAnnotate,
{
    pub annos: Vec<(&'a str, &'a InstanceAnnotations<T>)>,
    pub labelinfo: &'a LabelInfo,
}

type Attributes<'a> = Vec<(&'a str, &'a ParamMap)>;

#[derive(Serialize, Clone)]
pub struct WandManyAnnotationsInput<'a> {
    pub bbox: Option<AnnosWithInfo<'a, GeoFig>>,
    pub brush: Option<AnnosWithInfo<'a, Canvas>>,
    pub attributes: Option<Attributes<'a>>,
}

impl<'a> WandManyAnnotationsInput<'a> {
    pub fn from_tdm(
        tools_data_map: &'a ToolsDataMap,
        files: &'a [String],
        folders_to_exclude: &'a [String],
    ) -> (Self, Vec<&'a String>) {
        let files_wo_excluded_folders = files
            .iter()
            .filter(|f| {
                !folders_to_exclude
                    .iter()
                    .any(|excluded| Path::new(f).ancestors().any(|a| a.ends_with(excluded)))
            })
            .collect::<Vec<_>>();
        macro_rules! collect {
            ($tool_name:expr, $T:ty, $access:ident) => {
                tools_data_map
                    .get_specifics($tool_name)
                    .and_then(|s| {
                        s.$access()
                            .map(|bb| (bb.annotations_map(), bb.label_info()))
                            .ok()
                    })
                    .map(|(am, li)| {
                        (
                            files_wo_excluded_folders
                                .iter()
                                .flat_map(|f| am.get(f).map(|(annos, _)| (f.as_str(), annos)))
                                .collect::<Vec<(&str, &InstanceAnnotations<$T>)>>(),
                            li,
                        )
                    })
                    .map(|(annos, labelinfo)| AnnosWithInfo { annos, labelinfo })
            };
        }

        let bbox = collect!(BBOX_NAME, GeoFig, bbox);

        let brush = collect!(BRUSH_NAME, Canvas, brush);
        let attributes = tools_data_map.get_specifics(ATTRIBUTES_NAME).and_then(|s| {
            s.attributes()
                .map(|at| {
                    at.annotations_map
                        .iter()
                        .map(|(k, (params, _))| (k.as_str(), params))
                        .collect::<Attributes>()
                })
                .ok()
        });
        (
            Self {
                bbox,
                brush,
                attributes,
            },
            files_wo_excluded_folders,
        )
    }
}

pub type BboxOutput = Option<Vec<(String, (InstanceAnnotations<GeoFig>, ShapeI))>>;
pub type BrushOutput = Option<Vec<(String, (InstanceAnnotations<Canvas>, ShapeI))>>;
pub type AttributesOutput = Option<Vec<(String, (ParamMapUntagged, ShapeI))>>;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct WandManyOutput {
    pub bbox: BboxOutput,
    pub brush: BrushOutput,
    pub attributes: AttributesOutput,
}
impl WandManyOutput {
    pub fn resolve_into_tdm(self, tools_data_map: &mut ToolsDataMap) -> RvResult<()> {
        if let Some(bbox) = self.bbox
            && let Some(s) = tools_data_map.get_specifics_mut(BBOX_NAME)
            && let Some(bbox_data) = trace_ok_err(s.bbox_mut())
        {
            bbox_data.set_annotations_map(LabelMap::from_iter(bbox))?;
        }
        if let Some(brush) = self.brush
            && let Some(s) = tools_data_map.get_specifics_mut(BRUSH_NAME)
            && let Some(brush_data) = trace_ok_err(s.brush_mut())
        {
            brush_data.set_annotations_map(LabelMap::from_iter(brush))?;
        }
        if let Some(attributes) = self.attributes
            && let Some(s) = tools_data_map.get_specifics_mut(ATTRIBUTES_NAME)
            && let Some(attributes_data) = trace_ok_err(s.attributes_mut())
        {
            for (filename, (attributes_of_file, shape)) in attributes {
                attributes_data
                    .annotations_map
                    .insert(filename, (ParamMap::from(attributes_of_file), shape));
            }
        }
        Ok(())
    }
}

pub trait WandMany {
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
        prj_name: &'a str,
        annotations_input: WandManyAnnotationsInput<'a>,
        files: &[&String],
        selected_file_idx: Option<usize>,
        communication: &[WandManyMessage],
        parameters: Option<&ParamMap>,
    ) -> RvResult<(WandManyOutput, String)>;
}

#[derive(Serialize)]
struct RestWandQueryParams {
    prj_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_file_idx: Option<usize>,
}

pub struct RestWandMany {
    data: RestData,
}
impl RestWandMany {
    pub fn new(url: String, authorization: Option<&str>, timeout_s: usize) -> Self {
        Self {
            data: RestData::new(url, authorization, timeout_s * 1000, "predict_many"),
        }
    }
}

impl WandMany for RestWandMany {
    fn predict<'a>(
        &self,
        prj_name: &'a str,
        annos_input: WandManyAnnotationsInput<'a>,
        files: &[&String],
        selected_file_idx: Option<usize>,
        communication: &[WandManyMessage],
        parameters: Option<&ParamMap>,
    ) -> RvResult<(WandManyOutput, String)> {
        let annos_json_str = serde_json::to_string(&annos_input).map_err(to_rv)?;
        let param_json_str = serialize_or_default(parameters)?;
        let files_json_str = serde_json::to_string(files).map_err(to_rv)?;
        let communication_json_str = serde_json::to_string(communication).map_err(to_rv)?;
        let query_params = RestWandQueryParams {
            prj_name: prj_name.to_string(),
            selected_file_idx,
        };
        let form = multipart::Form::new()
            .part("input_annotations", multipart::Part::text(annos_json_str))
            .part("files", multipart::Part::text(files_json_str))
            .part(
                "communication",
                multipart::Part::text(communication_json_str),
            )
            .part("parameters", multipart::Part::text(param_json_str));
        self.data.send(form, Some(&query_params))
    }
}

#[cfg(test)]
use crate::{defer, parameters::ParamVal, test_helpers::start_resttestserver};
#[cfg(test)]
use rvimage_domain::{BbF, BbI};
#[cfg(test)]
use std::{thread, time::Duration};

#[test]
fn test_testserver() {
    let (_, mut child) = start_resttestserver();
    defer!(|| child.kill().expect("Failed to kill the server"));
    thread::sleep(Duration::from_secs(10));
    let url = "http://127.0.0.1:8000/";
    let w = RestWandMany::new(url.into(), None, 60000);
    let bbox_annos = InstanceAnnotations::from_elts_cats(
        vec![GeoFig::BB(BbF::from_arr(&[0.0, 0.0, 5.0, 5.0]))],
        vec![1],
    );
    let c = Canvas::from_box(BbI::from_arr(&[11, 11, 5, 5]), 1.0);
    let brush_annos = InstanceAnnotations::from_elts_cats(vec![c], vec![1]);
    let labelinfo = LabelInfo::default();
    let bbox_dummy = AnnosWithInfo {
        annos: vec![("file1.png", &bbox_annos)],
        labelinfo: &labelinfo,
    };
    let brush_dummy = AnnosWithInfo {
        annos: vec![("file1.png", &brush_annos)],
        labelinfo: &labelinfo,
    };
    let pm = ParamMap::from([("param_name".to_string(), ParamVal::from(1))]);
    let attributes = vec![("filename", &pm)];
    let annos = WandManyAnnotationsInput {
        bbox: Some(bbox_dummy),
        brush: Some(brush_dummy),
        attributes: Some(attributes),
    };
    let mut param_map = ParamMap::new();
    param_map.insert("a".to_string(), ParamVal::from(42));
    param_map.insert("c".to_string(), ParamVal::from(true));
    param_map.insert("d".to_string(), ParamVal::from("thestr".to_string()));
    let (output, s) = w
        .predict("dummy", annos, &[], None, &[], Some(&param_map))
        .unwrap();
    assert_eq!("method_description", s);
    println!("attributes {:?}", output.attributes);
}
