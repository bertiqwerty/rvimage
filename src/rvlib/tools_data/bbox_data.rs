use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_pickle::{DeOptions, SerOptions};

use crate::{
    annotations::BboxAnnotations,
    domain::BB,
    file_util::{ExportData, MetaData},
    format_rverr, implement_annotations_getters,
    result::{to_rv, RvError, RvResult},
};

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (c1[i] as f32 - c2[i] as f32).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

fn random_clr() -> [u8; 3] {
    let r = rand::random::<u8>();
    let g = rand::random::<u8>();
    let b = rand::random::<u8>();
    [r, g, b]
}

fn argmax_clr_dist(picklist: &[[u8; 3]], legacylist: &[[u8; 3]]) -> [u8; 3] {
    let (idx, _) = picklist
        .iter()
        .enumerate()
        .map(|(i, pickclr)| {
            let min_dist = legacylist
                .iter()
                .map(|legclr| color_dist(*legclr, *pickclr))
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            (i, min_dist)
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    picklist[idx]
}

fn new_color(colors: &[[u8; 3]]) -> [u8; 3] {
    let mut new_clr_proposals = [[0u8, 0u8, 0u8]; 10];
    for new_clr in &mut new_clr_proposals {
        *new_clr = random_clr();
    }
    argmax_clr_dist(&new_clr_proposals, colors)
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, PartialEq, Eq)]
pub enum BboxExportFileType {
    Json,
    Pickle,
    #[default]
    None,
}
static DEFAULT_BBOX_ANNOTATION: BboxAnnotations = BboxAnnotations::new();
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct BboxSpecificData {
    pub new_label: String,
    labels: Vec<String>,
    colors: Vec<[u8; 3]>,
    pub cat_id_current: usize,
    // filename -> annotations per file
    annotations_map: HashMap<String, BboxAnnotations>,
    pub export_file_type: BboxExportFileType,
    pub import_file: Option<String>,
}
impl BboxSpecificData {
    implement_annotations_getters!(&DEFAULT_BBOX_ANNOTATION, BboxAnnotations);
    pub fn remove_cat(&mut self, cat_id: usize) {
        if self.labels.len() > 1 {
            self.labels.remove(cat_id);
            self.colors.remove(cat_id);
            if self.cat_id_current >= cat_id.max(1) {
                self.cat_id_current -= 1;
            }
            for anno in self.annotations_map.values_mut() {
                anno.remove_cat(cat_id);
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        self.colors.len() == 0
    }
    pub fn len(&self) -> usize {
        self.colors.len()
    }
    pub fn find_empty(&mut self) -> Option<&mut String> {
        self.labels.iter_mut().find(|lab| lab == &"")
    }
    pub fn push(&mut self, label: String, color: Option<[u8; 3]>) {
        if let Some(idx) = self.labels.iter().position(|lab| lab == &label) {
            if let Some(clr) = color {
                self.colors[idx] = clr;
            }
        } else {
            self.labels.push(label);
            if let Some(clr) = color {
                self.colors.push(clr);
            } else {
                let new_clr = new_color(&self.colors);
                self.colors.push(new_clr);
            }
        }
    }
    pub fn colors(&self) -> &Vec<[u8; 3]> {
        &self.colors
    }
    pub fn labels(&self) -> &Vec<String> {
        &self.labels
    }
    pub fn new() -> Self {
        let new_label = "".to_string();
        let new_color = [255, 255, 255];
        let labels = vec![new_label.clone()];
        let colors = vec![new_color];
        BboxSpecificData {
            new_label,
            labels,
            colors,
            cat_id_current: 0,
            annotations_map: HashMap::new(),
            export_file_type: BboxExportFileType::default(),
            import_file: None,
        }
    }
    pub fn set_annotations_map(&mut self, map: HashMap<String, BboxAnnotations>) -> RvResult<()> {
        for (_, annos) in map.iter() {
            for cat_id in annos.cat_ids() {
                let len = self.labels().len();
                if *cat_id >= len {
                    return Err(format_rverr!(
                        "cat id {} does not have a label, out of bounds, {}",
                        cat_id,
                        len
                    ));
                }
            }
        }
        self.annotations_map = map;
        Ok(())
    }
}
impl Default for BboxSpecificData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct BboxExportData {
    pub labels: Vec<String>,
    pub colors: Vec<[u8; 3]>,
    pub annotations: HashMap<String, (Vec<BB>, Vec<usize>)>,
}

impl BboxExportData {
    pub fn to_bbox_data(self) -> RvResult<BboxSpecificData> {
        let mut bbox_data = BboxSpecificData::new();
        for (lab, clr) in self.labels.into_iter().zip(self.colors.into_iter()) {
            bbox_data.push(lab, Some(clr));
        }
        bbox_data.remove_cat(0);
        bbox_data.set_annotations_map(
            self.annotations
                .into_iter()
                .map(|(s, (bbs, cat_ids))| (s, BboxAnnotations::from_bbs_cats(bbs, cat_ids)))
                .collect(),
        )?;
        Ok(bbox_data)
    }
}
fn get_last_part_of_path(path: &str, sep: char) -> Option<String> {
    if path.contains(sep) {
        let mark = if path.starts_with('\'') && path.ends_with('\'') {
            "\'"
        } else if path.starts_with('"') && path.ends_with('"') {
            "\""
        } else {
            ""
        };
        let offset = mark.len();
        let mut fp_slice = &path[offset..(path.len() - offset)];
        let mut last_folder = fp_slice.split(sep).last().unwrap_or("");
        while last_folder.is_empty() && !fp_slice.is_empty() {
            fp_slice = &fp_slice[0..fp_slice.len() - 1];
            last_folder = fp_slice.split(sep).last().unwrap_or("");
        }
        Some(format!("{}{}{}", mark, last_folder, mark))
    } else {
        None
    }
}

fn write(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
    extension: &str,
    ser: fn(&ExportData, &Path) -> RvResult<()>,
) -> RvResult<PathBuf> {
    let ef = meta_data
        .export_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no export folder given"))?;
    let ef_path = Path::new(ef);
    match fs::create_dir_all(ef_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format_rverr!(
            "could not create {:?} due to {:?}",
            ef_path,
            e
        )),
    }?;

    let of = meta_data
        .opened_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no folder opened"))?;
    let data = ExportData {
        opened_folder: of.clone(),
        connection_data: meta_data.connection_data.clone(),
        bbox_data: Some(BboxExportData {
            labels: bbox_specifics.labels().clone(),
            colors: bbox_specifics.colors().clone(),
            annotations: bbox_specifics
                .anno_iter()
                .map(|(filename, annos)| {
                    (
                        filename.clone(),
                        (annos.bbs().clone(), annos.cat_ids().clone()),
                    )
                })
                .collect::<HashMap<_, _>>(),
        }),
    };
    let of_last_part_linux = get_last_part_of_path(of, '/');
    let of_last_part_windows =
        get_last_part_of_path(of_last_part_linux.as_ref().unwrap_or(of), '\\');
    let of_last_part =
        of_last_part_windows.unwrap_or_else(|| of_last_part_linux.unwrap_or_else(|| of.clone()));
    let path = Path::new(ef_path)
        .join(of_last_part)
        .with_extension(extension);
    ser(&data, &path).map_err(to_rv)?;

    println!("exported labels to {:?}", path);
    Ok(path)
}

pub fn write_json(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<PathBuf> {
    let ser = |data: &ExportData, path: &Path| {
        let data_str = serde_json::to_string(&data).map_err(to_rv)?;
        fs::write(&path, data_str).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "json", ser)
}

fn _convert_read(read: ExportData) -> RvResult<BboxSpecificData> {
    let bb_read = read
        .bbox_data
        .ok_or_else(|| RvError::new("import does not contain bbox data"))?;
    bb_read.to_bbox_data()
}

pub fn _read_json(filename: &str) -> RvResult<BboxSpecificData> {
    let s = fs::read_to_string(filename).map_err(to_rv)?;
    let read: ExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    _convert_read(read)
}

pub fn write_pickle(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<PathBuf> {
    let ser = |data: &ExportData, path: &Path| {
        let mut file = File::create(path).map_err(to_rv)?;
        serde_pickle::to_writer(&mut file, data, SerOptions::new()).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "pickle", ser)
}

pub fn _read_pickle(filename: &str) -> RvResult<BboxSpecificData> {
    let f = File::open(filename).map_err(to_rv)?;
    let read: ExportData = serde_pickle::from_reader(f, DeOptions::new()).map_err(to_rv)?;
    _convert_read(read)
}
#[cfg(test)]
use crate::{
    cfg::SshCfg,
    domain::make_test_bbs,
    file_util,
    {defer_file_removal, file_util::DEFAULT_TMPDIR},
};
#[cfg(test)]
fn make_data(extension: &str) -> (BboxSpecificData, MetaData, PathBuf) {
    use crate::file_util::ConnectionData;

    let opened_folder = "xi".to_string();
    let test_export_folder = DEFAULT_TMPDIR.clone();

    match fs::create_dir(&test_export_folder) {
        Ok(_) => (),
        Err(e) => {
            println!("{:?}", e);
        }
    }

    let test_export_path = DEFAULT_TMPDIR.join(format!("{}.{}", opened_folder, extension));
    let mut meta = MetaData::from_filepath(
        test_export_path
            .with_extension("egal")
            .to_str()
            .unwrap()
            .to_string(),
    );
    meta.opened_folder = Some(opened_folder);
    meta.export_folder = Some(test_export_folder.to_str().unwrap().to_string());
    meta.connection_data = ConnectionData::Ssh(SshCfg::default());
    let mut bbox_data = BboxSpecificData::new();
    bbox_data.push("x".to_string(), None);
    bbox_data.remove_cat(0);
    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    let key = "dummyfile";
    let annos = bbox_data.get_annos_mut(key);
    for bb in bbs {
        annos.add_bb(bb, 0);
    }
    (bbox_data, meta, test_export_path)
}
#[test]
fn test_json_export() -> RvResult<()> {
    let (bbox_data, meta, path) = make_data("json");
    defer_file_removal!(&path);
    let written_path = write_json(&meta, bbox_data.clone())?;
    let bbox_data_read =
        _read_json(file_util::osstr_to_str(Some(written_path.as_os_str())).map_err(to_rv)?)?;
    assert_eq!(bbox_data, bbox_data_read);
    Ok(())
}
#[test]
fn test_pickle_export() -> RvResult<()> {
    let (bbox_data, meta, path) = make_data("pickle");
    defer_file_removal!(&path);
    let written_path = write_pickle(&meta, bbox_data.clone())?;
    let bbox_data_read =
        _read_pickle(file_util::osstr_to_str(Some(written_path.as_os_str())).map_err(to_rv)?)?;
    assert_eq!(bbox_data, bbox_data_read);
    Ok(())
}
#[test]
fn last_folder_part() {
    assert_eq!(get_last_part_of_path("a/b/c", '/'), Some("c".to_string()));
    assert_eq!(get_last_part_of_path("a/b/c", '\\'), None);
    assert_eq!(get_last_part_of_path("a\\b\\c", '/'), None);
    assert_eq!(
        get_last_part_of_path("a\\b\\c", '\\'),
        Some("c".to_string())
    );
    assert_eq!(get_last_part_of_path("", '/'), None);
    assert_eq!(get_last_part_of_path("a/b/c/", '/'), Some("c".to_string()));
    assert_eq!(
        get_last_part_of_path("aadfh//bdafl////aksjc/////", '/'),
        Some("aksjc".to_string())
    );
    assert_eq!(
        get_last_part_of_path("\"aa dfh//bdafl////aks jc/////\"", '/'),
        Some("\"aks jc\"".to_string())
    );
    assert_eq!(
        get_last_part_of_path("'aa dfh//bdafl////aks jc/////'", '/'),
        Some("'aks jc'".to_string())
    );
}

#[test]
fn test_argmax() {
    let picklist = [
        [200, 200, 200u8],
        [1, 7, 3],
        [0, 0, 1],
        [45, 43, 52],
        [1, 10, 15],
    ];
    let legacylist = [
        [17, 16, 15],
        [199, 199, 201u8],
        [50, 50, 50u8],
        [255, 255, 255u8],
    ];
    assert_eq!(argmax_clr_dist(&picklist, &legacylist), [0, 0, 1]);
}
