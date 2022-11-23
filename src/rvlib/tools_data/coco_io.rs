use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    domain::BB,
    file_util::{self, MetaData},
    result::{to_rv, RvError, RvResult},
};

use super::{BboxExportData, BboxSpecificData};

#[derive(Serialize, Deserialize)]
struct CocoInfo {
    description: String,
}

#[derive(Serialize, Deserialize)]
struct CocoImage {
    id: u32,
    width: u32,
    height: u32,
    file_name: String,
}

#[derive(Serialize, Deserialize)]
struct CocoBboxCategory {
    id: u32,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct CocoAnnotation {
    id: u32,
    image_id: u32,
    category_id: u32,
    segmentation: Vec<Vec<u32>>,
    area: f32,
    bbox: [u32; 4],
    iscrowd: u8,
}

#[derive(Serialize, Deserialize)]
struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}

pub fn write_coco(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<PathBuf> {
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
    let file_name = format!("{}_coco.json", opened_folder);
    let coco_out_path = export_folder.join(file_name);
    let coco_data = bboxdata_to_coco(meta_data, bbox_specifics)?;
    let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
    file_util::write(&coco_out_path, data_str)?;
    println!("exported coco labels to {:?}", coco_out_path);
    Ok(coco_out_path)
}
fn bboxdata_to_coco(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
) -> RvResult<CocoExportData> {
    let opened_folder = meta_data
        .opened_folder
        .as_deref()
        .ok_or_else(|| RvError::new("no folder open"))?;

    let info_str = "created with Rvimage, https://github.com/bertiqwerty/rvimage".to_string();
    let info = CocoInfo {
        description: info_str,
    };
    let export_data = BboxExportData::from_bbox_data(bbox_specifics);

    let make_image_map = |(idx, filename)| {
        let file_path = Path::new(opened_folder)
            .join(filename)
            .into_os_string()
            .into_string()
            .map_err(to_rv)?;
        let (w, h) = image::image_dimensions(&file_path).map_err(to_rv)?;
        Ok(CocoImage {
            id: idx as u32,
            width: w,
            height: h,
            file_name: file_path,
        })
    };
    let images = export_data
        .annotations
        .keys()
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
    let make_anno_map = |(image_idx, (bbs, cat_idxs)): (usize, &(Vec<BB>, Vec<usize>))| {
        bbs.iter()
            .zip(cat_idxs.iter())
            .map(|(bb, cat_idx): (&BB, &usize)| {
                box_id += 1;
                CocoAnnotation {
                    id: box_id - 1,
                    image_id: image_idx as u32,
                    category_id: export_data.cat_ids[*cat_idx] as u32,
                    bbox: [bb.x, bb.y, bb.w, bb.h],
                    segmentation: vec![],
                    area: (bb.h * bb.w) as f32,
                    iscrowd: 0,
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

#[cfg(test)]
use {
    super::bbox_data::make_data,
    crate::{cfg::get_cfg, defer_file_removal, types::ViewImage},
    std::{fs, str::FromStr},
};

#[test]
fn test_coco_export() -> RvResult<()> {
    let image = ViewImage::new(32, 32);
    let tmpdir = get_cfg()?.tmpdir().unwrap().to_string();
    let tmpdir = PathBuf::from_str(&tmpdir).unwrap();
    fs::create_dir_all(&tmpdir).unwrap();
    let file_path = tmpdir.join("test_image.png");
    image.save(&file_path).unwrap();
    defer_file_removal!(&file_path);
    let (bbox_data, meta, _) = make_data("json", &file_path);
    let coco_file = write_coco(&meta, bbox_data.clone())?;
    defer_file_removal!(&coco_file);
    Ok(())
}
