use std::{
    collections::HashMap,
    fmt::Debug,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    domain::{Shape, BB},
    file_util::{self, MetaData},
    result::{to_rv, RvError, RvResult},
    rverr,
};

use super::{
    bbox_data::{new_color, random_clr},
    BboxExportData, BboxSpecificData,
};

#[derive(Serialize, Deserialize, Debug)]
struct CocoInfo {
    description: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoImage {
    id: u32,
    width: u32,
    height: u32,
    file_name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoBboxCategory {
    id: u32,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoAnnotation {
    id: u32,
    image_id: u32,
    category_id: u32,
    bbox: [f32; 4],
    segmentation: Option<Vec<Vec<f32>>>,
    area: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}
impl CocoExportData {
    fn from_coco(bbox_specifics: BboxSpecificData) -> RvResult<Self> {
        let info_str = "created with Rvimage, https://github.com/bertiqwerty/rvimage".to_string();
        let info = CocoInfo {
            description: info_str,
        };
        let export_data = BboxExportData::from_bbox_data(bbox_specifics);

        type AnnotationMapValue<'a> = (&'a String, &'a (Vec<BB>, Vec<usize>, Shape));
        let make_image_map = |(idx, (file_path, (_, _, shape))): (usize, AnnotationMapValue)| {
            Ok(CocoImage {
                id: idx as u32,
                width: shape.w,
                height: shape.h,
                file_name: file_path.clone(),
            })
        };
        let images = export_data
            .annotations
            .iter()
            .enumerate()
            .map(make_image_map)
            .collect::<RvResult<Vec<_>>>()?;

        let categories = export_data
            .labels
            .iter()
            .zip(export_data.cat_ids.iter())
            .map(|(label, cat_id)| CocoBboxCategory {
                id: *cat_id as u32,
                name: label.clone(),
            })
            .collect::<Vec<_>>();

        let mut box_id = 0;
        let make_anno_map =
            |(image_idx, (bbs, cat_idxs, _)): (usize, &(Vec<BB>, Vec<usize>, Shape))| {
                bbs.iter()
                    .zip(cat_idxs.iter())
                    .map(|(bb, cat_idx): (&BB, &usize)| {
                        let bb_f = [bb.x as f32, bb.y as f32, bb.w as f32, bb.h as f32];
                        box_id += 1;
                        CocoAnnotation {
                            id: box_id - 1,
                            image_id: image_idx as u32,
                            category_id: export_data.cat_ids[*cat_idx] as u32,
                            bbox: bb_f,
                            segmentation: Some(vec![vec![
                                bb_f[0],
                                bb_f[1],
                                bb_f[0] + bb_f[2],
                                bb_f[1],
                                bb_f[0] + bb_f[2],
                                bb_f[1] + bb_f[3],
                                bb_f[0],
                                bb_f[1] + bb_f[3],
                            ]]),
                            area: Some((bb.h * bb.w) as f32),
                        }
                    })
                    .collect::<Vec<_>>()
            };
        let annotations = export_data
            .annotations
            .values()
            .enumerate()
            .flat_map(make_anno_map)
            .collect::<Vec<_>>();

        Ok(CocoExportData {
            info,
            images,
            annotations,
            categories,
        })
    }

    fn convert_to_bboxdata(self) -> RvResult<BboxSpecificData> {
        let cat_ids: Vec<u32> = self.categories.iter().map(|coco_cat| coco_cat.id).collect();
        let labels: Vec<String> = self
            .categories
            .into_iter()
            .map(|coco_cat| coco_cat.name)
            .collect();
        let mut colors: Vec<[u8; 3]> = vec![random_clr()];
        for _ in 0..(labels.len() - 1) {
            let color = new_color(&colors);
            colors.push(color);
        }
        let id_image_map = self
            .images
            .iter()
            .map(|coco_image: &CocoImage| {
                Ok((
                    coco_image.id,
                    (
                        coco_image.file_name.as_str(),
                        coco_image.width,
                        coco_image.height,
                    ),
                ))
            })
            .collect::<RvResult<HashMap<u32, (&str, u32, u32)>>>()?;

        let mut annotations: HashMap<String, (Vec<BB>, Vec<usize>, Shape)> = HashMap::new();
        for coco_anno in self.annotations {
            let (file_name, w, h) = id_image_map[&coco_anno.image_id];

            let (w_factor, h_factor) = if coco_anno.bbox.iter().all(|x| *x <= 1.0) {
                (w as f32, h as f32)
            } else {
                (1.0, 1.0)
            };
            let bbox = [
                (w_factor * coco_anno.bbox[0]).round() as u32,
                (h_factor * coco_anno.bbox[1]).round() as u32,
                (w_factor * coco_anno.bbox[2]).round() as u32,
                (h_factor * coco_anno.bbox[3]).round() as u32,
            ];

            let bb = BB::from_array(&bbox);
            let cat_idx = cat_ids
                .iter()
                .position(|cat_id| *cat_id == coco_anno.category_id)
                .ok_or_else(|| {
                    rverr!(
                        "could not find cat id {}, we only have {:?}",
                        coco_anno.category_id,
                        cat_ids
                    )
                })?;
            let k: &str = file_name;
            if let Some(annos_of_image) = annotations.get_mut(k) {
                annos_of_image.0.push(bb);
                annos_of_image.1.push(cat_idx);
            } else {
                annotations.insert(k.to_string(), (vec![bb], vec![cat_idx], Shape::new(w, h)));
            }
        }
        BboxSpecificData::from_bbox_export_data(BboxExportData {
            labels,
            colors,
            cat_ids,
            annotations,
        })
    }
}

fn meta_data_to_coco_path(meta_data: &MetaData) -> RvResult<PathBuf> {
    let export_folder = Path::new(
        meta_data
            .export_folder
            .as_ref()
            .ok_or_else(|| RvError::new("no export folder given"))?,
    );
    let opened_folder = meta_data
        .opened_folder
        .as_deref()
        .ok_or_else(|| RvError::new("no folder open"))?;
    let parent = Path::new(opened_folder)
        .parent()
        .and_then(|p| p.file_stem())
        .and_then(|p| p.to_str());

    let opened_folder_name = Path::new(opened_folder)
        .file_stem()
        .and_then(|of| of.to_str())
        .ok_or_else(|| rverr!("cannot find folder name  of {}", opened_folder))?;
    let file_name = if let Some(p) = parent {
        format!("{}_{}_coco.json", p, opened_folder_name)
    } else {
        format!("{}_coco.json", opened_folder_name)
    };
    Ok(export_folder.join(file_name))
}

pub fn write_coco(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<PathBuf> {
    let coco_data = CocoExportData::from_coco(bbox_specifics)?;
    let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
    let coco_out_path = meta_data_to_coco_path(meta_data)?;
    file_util::write(&coco_out_path, data_str)?;
    println!("exported coco labels to {:?}", coco_out_path);
    Ok(coco_out_path)
}

pub fn read_coco(meta_data: &MetaData) -> RvResult<BboxSpecificData> {
    let filename = meta_data_to_coco_path(meta_data)?;
    let s = file_util::read_to_string(&filename)?;
    let read: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    println!("imported coco file from {:?}", filename);
    read.convert_to_bboxdata()
}

#[cfg(test)]
use {
    crate::{
        cfg::{get_cfg, SshCfg},
        defer_file_removal,
        domain::make_test_bbs,
    },
    file_util::{ConnectionData, DEFAULT_TMPDIR},
    std::{fs, str::FromStr},
};

#[cfg(test)]
pub fn make_data(
    extension: &str,
    image_file: &Path,
    opened_folder: Option<&Path>,
) -> (BboxSpecificData, MetaData, PathBuf) {
    let opened_folder = if let Some(of) = opened_folder {
        of.to_str().unwrap().to_string()
    } else {
        "xi".to_string()
    };
    let test_export_folder = DEFAULT_TMPDIR.clone();

    if !test_export_folder.exists() {
        match fs::create_dir(&test_export_folder) {
            Ok(_) => (),
            Err(e) => {
                println!("{:?}", e);
            }
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
    bbox_data.push("x".to_string(), None, None).unwrap();
    bbox_data.remove_catidx(0);
    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());

    let annos =
        bbox_data.get_annos_mut(image_file.as_os_str().to_str().unwrap(), Shape::new(10, 10));
    for bb in bbs {
        annos.add_bb(bb, 0);
    }
    (bbox_data, meta, test_export_path)
}

#[test]
fn test_coco_export() -> RvResult<()> {
    fn test(file_path: &Path, opened_folder: Option<&Path>) -> RvResult<()> {
        let (bbox_data, meta, _) = make_data("json", &file_path, opened_folder);
        let coco_file = write_coco(&meta, bbox_data.clone())?;
        defer_file_removal!(&coco_file);
        let read = read_coco(&meta)?;
        assert_eq!(bbox_data.cat_ids(), read.cat_ids());
        assert_eq!(bbox_data.labels(), read.labels());
        for (bbd_anno, read_anno) in bbox_data.anno_iter().zip(read.anno_iter()) {
            assert_eq!(bbd_anno, read_anno);
        }
        Ok(())
    }
    let tmpdir = get_cfg()?.tmpdir().unwrap().to_string();
    let tmpdir = PathBuf::from_str(&tmpdir).unwrap();
    if !tmpdir.exists() {
        // fs::create_dir_all(&tmpdir).unwrap();
    }
    let file_path = tmpdir.join("test_image.png");
    test(&file_path, None)?;
    let folder = Path::new("http://localhost:8000/some_path");
    let file = Path::new("http://localhost:8000/some_path/xyz.png");
    test(file, Some(folder))?;
    Ok(())
}

#[cfg(test)]
const TEST_DATA_FOLDER: &str = "resources/test_data/";

#[test]
fn test_coco_import() -> RvResult<()> {
    fn test(filename: &str, cat_ids: Vec<u32>, bbs: &[(BB, &str)]) {
        let meta = MetaData {
            file_path: None,
            connection_data: ConnectionData::None,
            opened_folder: Some(filename.to_string()),
            export_folder: Some(TEST_DATA_FOLDER.to_string()),
        };
        let read = read_coco(&meta).unwrap();
        assert_eq!(read.cat_ids(), &cat_ids);
        assert_eq!(read.labels(), &vec!["first label", "second label"]);
        for (bb, file_path) in bbs {
            let annos = read.get_annos(file_path);
            assert!(annos.unwrap().bbs().contains(bb));
        }
    }
    let bb_im_ref_abs = [
        (BB::from_array(&[1, 1, 5, 5]), "nowhere.png"),
        (BB::from_array(&[11, 11, 4, 7]), "nowhere.png"),
        (BB::from_array(&[1, 1, 5, 5]), "nowhere2.png"),
    ];
    let bb_im_ref_relative = [
        (BB::from_array(&[10, 100, 50, 500]), "nowhere.png"),
        (BB::from_array(&[91, 870, 15, 150]), "nowhere.png"),
        (BB::from_array(&[10, 1, 50, 5]), "nowhere2.png"),
    ];
    test("catids_12", vec![1, 2], &bb_im_ref_abs);
    test("catids_01", vec![0, 1], &bb_im_ref_abs);
    test("catids_12_relative", vec![1, 2], &bb_im_ref_relative);
    Ok(())
}
