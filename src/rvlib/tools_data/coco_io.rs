use std::{
    collections::HashMap,
    fmt::Debug,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    domain::BB,
    file_util::{self, MetaData},
    format_rverr,
    result::{to_rv, RvError, RvResult},
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
    segmentation: Vec<Vec<u32>>,
    area: f32,
    bbox: [u32; 4],
    iscrowd: u8,
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}
impl CocoExportData {
    fn from_coco(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<Self> {
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
            .map(|coco_image: &CocoImage| Ok((coco_image.id, coco_image.file_name.as_str())))
            .collect::<RvResult<HashMap<u32, &str>>>()?;

        let mut annotations: HashMap<String, (Vec<BB>, Vec<usize>)> = HashMap::new();
        for coco_anno in self.annotations {
            let bb = BB::from_array(&coco_anno.bbox);
            let cat_idx = cat_ids
                .iter()
                .position(|cat_id| *cat_id == coco_anno.category_id)
                .ok_or_else(|| {
                    format_rverr!(
                        "could not find cat id {}, we only have {:?}",
                        coco_anno.category_id,
                        cat_ids
                    )
                })?;
            let k: &str = id_image_map[&coco_anno.image_id];
            if let Some(annos_of_image) = annotations.get_mut(k) {
                annos_of_image.0.push(bb);
                annos_of_image.1.push(cat_idx);
            } else {
                annotations.insert(k.to_string(), (vec![bb], vec![cat_idx]));
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
    let coco_data = CocoExportData::from_coco(meta_data, bbox_specifics)?;
    let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
    file_util::write(&coco_out_path, data_str)?;
    println!("exported coco labels to {:?}", coco_out_path);
    Ok(coco_out_path)
}

pub fn read_coco<P>(filename: P) -> RvResult<BboxSpecificData>
where
    P: AsRef<Path> + Debug,
{
    let s = file_util::read_to_string(filename)?;
    let read: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;

    read.convert_to_bboxdata()
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
    let read = read_coco(&coco_file)?;
    assert_eq!(bbox_data.cat_ids(), read.cat_ids());
    assert_eq!(bbox_data.labels(), read.labels());
    for (bbd_anno, read_anno) in bbox_data.anno_iter().zip(read.anno_iter()) {
        assert_eq!(bbd_anno, read_anno);
    }
    Ok(())
}

#[test]
fn test_coco_import() -> RvResult<()> {
    fn test(filename: &str, cat_ids: Vec<u32>) -> RvResult<()> {
        let read = read_coco(filename)?;
        assert_eq!(read.cat_ids(), &cat_ids);
        assert_eq!(read.labels(), &vec!["first label", "second label"]);
        Ok(())
    }
    test("resources/test_data/coco_catids_12.json", vec![1, 2])?;
    test("resources/test_data/coco_catids_12.json", vec![1, 2])?;
    Ok(())
}
