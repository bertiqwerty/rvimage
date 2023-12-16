use std::{
    collections::HashMap,
    fmt::Debug,
    iter,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    cfg::{CocoFile, CocoFileConnection},
    domain::{Point, Shape, BB},
    file_util::{self, path_to_str, MetaData},
    result::{to_rv, RvError, RvResult},
    rverr, ssh, GeoFig, Polygon,
};

use super::{bbox_data::new_random_colors, BboxExportData, BboxSpecificData};

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

fn colors_to_string(colors: &[[u8; 3]]) -> Option<String> {
    colors
        .iter()
        .map(|[r, g, b]| format!("{r};{g};{b}"))
        .reduce(|s1, s2| format!("{s1}_{s2}"))
}

fn string_to_colors(s: &str) -> RvResult<Vec<[u8; 3]>> {
    let make_err = || rverr!("cannot convert str {} to rgb", s);
    s.trim()
        .split('_')
        .map(|rgb_str| {
            let mut rgb = [0; 3];
            let mut it = rgb_str.split(';');
            for c in &mut rgb {
                *c = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(make_err)?;
            }
            Ok(rgb)
        })
        .collect::<RvResult<Vec<[u8; 3]>>>()
}
#[derive(Serialize, Deserialize, Debug)]
struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}
impl CocoExportData {
    fn from_bboxdata(bbox_specifics: BboxSpecificData) -> RvResult<Self> {
        let color_str = if let Some(s) = colors_to_string(bbox_specifics.colors()) {
            format!(", {s}")
        } else {
            "".to_string()
        };
        let info_str =
            format!("created with Rvimage, https://github.com/bertiqwerty/rvimage{color_str}");
        let info = CocoInfo {
            description: info_str,
        };
        let export_data = BboxExportData::from_bbox_data(bbox_specifics);

        type AnnotationMapValue<'a> = (&'a String, &'a (Vec<GeoFig>, Vec<usize>, Shape));
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
                id: *cat_id,
                name: label.clone(),
            })
            .collect::<Vec<_>>();

        let mut box_id = 0;
        let make_anno_map =
            |(image_idx, (bbs, cat_idxs, shape)): (usize, &(Vec<GeoFig>, Vec<usize>, Shape))| {
                bbs.iter()
                    .zip(cat_idxs.iter())
                    .map(|(geo, cat_idx): (&GeoFig, &usize)| {
                        let bb = geo.enclosing_bb();

                        let (imw, imh) = if export_data.is_export_absolute {
                            (1.0, 1.0)
                        } else {
                            (shape.w as f32, shape.h as f32)
                        };
                        let segmentation = geo.points_normalized(imw, imh);
                        let segmentation = segmentation
                            .iter()
                            .flat_map(|p| iter::once(p.x).chain(iter::once(p.y)))
                            .collect::<Vec<_>>();
                        let bb_f = [
                            bb.x as f32 / imw,
                            bb.y as f32 / imh,
                            bb.w as f32 / imw,
                            bb.h as f32 / imh,
                        ];
                        box_id += 1;
                        CocoAnnotation {
                            id: box_id - 1,
                            image_id: image_idx as u32,
                            category_id: export_data.cat_ids[*cat_idx],
                            bbox: bb_f,
                            segmentation: Some(vec![segmentation]),
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

    fn convert_to_bboxdata(self, coco_file: CocoFile) -> RvResult<BboxSpecificData> {
        let cat_ids: Vec<u32> = self.categories.iter().map(|coco_cat| coco_cat.id).collect();
        let labels: Vec<String> = self
            .categories
            .into_iter()
            .map(|coco_cat| coco_cat.name)
            .collect();
        let color_str = self.info.description.split(',').last();
        let colors: Vec<[u8; 3]> = if let Some(s) = color_str {
            string_to_colors(s).unwrap_or_else(|_| new_random_colors(labels.len()))
        } else {
            new_random_colors(labels.len())
        };
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

        let mut annotations: HashMap<String, (Vec<GeoFig>, Vec<usize>, Shape)> = HashMap::new();
        for coco_anno in self.annotations {
            let (file_name, w, h) = id_image_map[&coco_anno.image_id];

            let coords_absolute = coco_anno.bbox.iter().any(|x| *x > 1.0);
            let (w_factor, h_factor) = if coords_absolute {
                (1.0, 1.0)
            } else {
                (w as f32, h as f32)
            };
            let bbox = [
                (w_factor * coco_anno.bbox[0]).round() as u32,
                (h_factor * coco_anno.bbox[1]).round() as u32,
                (w_factor * coco_anno.bbox[2]).round() as u32,
                (h_factor * coco_anno.bbox[3]).round() as u32,
            ];
            let bb = BB::from_arr(&bbox);
            let geo = if let Some(segmentation) = coco_anno.segmentation {
                if !segmentation.is_empty() {
                    if segmentation.len() > 1 {
                        tracing::error!(
                            "multiple polygons per box not supported. ignoring all but first."
                        )
                    }
                    let n_points = segmentation[0].len();
                    let coco_data = &segmentation[0];
                    let poly = Polygon::from_vec(
                        (0..n_points)
                            .step_by(2)
                            .map(|idx| Point {
                                x: (coco_data[idx] * w_factor).round() as u32,
                                y: (coco_data[idx + 1] * h_factor).round() as u32,
                            })
                            .collect(),
                    );
                    match poly {
                        Ok(poly) => {
                            let encl_bb = poly.enclosing_bb();

                            // check if the poly is just a bounding box
                            if poly.points().len() == 4
                                // all points are bb corners
                                && poly.points_iter().all(|p| {
                                    encl_bb.points_iter().any(|p_encl| p == p_encl)})
                                // all points are different
                                && poly
                                    .points_iter()
                                    .all(|p| poly.points_iter().filter(|p_| p == *p_).count() == 1)
                            {
                                GeoFig::BB(poly.enclosing_bb())
                            } else {
                                GeoFig::Poly(poly)
                            }
                        }
                        Err(_) => {
                            // polygon might be empty, we continue with the BB
                            GeoFig::BB(bb)
                        }
                    }
                } else {
                    GeoFig::BB(bb)
                }
            } else {
                GeoFig::BB(bb)
            };

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
            let k = if file_name.starts_with("http") {
                file_util::url_encode(file_name)
            } else {
                file_name.to_string()
            };
            if let Some(annos_of_image) = annotations.get_mut(&k) {
                annos_of_image.0.push(geo);
                annos_of_image.1.push(cat_idx);
            } else {
                annotations.insert(k, (vec![geo], vec![cat_idx], Shape::new(w, h)));
            }
        }
        BboxSpecificData::from_bbox_export_data(BboxExportData {
            labels,
            colors,
            cat_ids,
            annotations,
            coco_file,
            is_export_absolute: false,
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
        format!("{p}_{opened_folder_name}_coco.json")
    } else {
        format!("{opened_folder_name}_coco.json")
    };
    Ok(export_folder.join(file_name))
}
fn get_cocofilepath(meta_data: &MetaData, coco_file: &CocoFile) -> RvResult<PathBuf> {
    if path_to_str(&coco_file.path)?.is_empty() {
        meta_data_to_coco_path(meta_data)
    } else {
        Ok(coco_file.path.clone())
    }
}
pub fn write_coco(meta_data: &MetaData, bbox_specifics: BboxSpecificData) -> RvResult<PathBuf> {
    let coco_out_path = get_cocofilepath(meta_data, &bbox_specifics.coco_file)?;
    let conn = bbox_specifics.coco_file.conn.clone();
    let coco_data = CocoExportData::from_bboxdata(bbox_specifics)?;
    let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
    match conn {
        CocoFileConnection::Ssh => {
            if let Some(ssh_cfg) = &meta_data.ssh_cfg {
                let sess = ssh::auth(ssh_cfg)?;
                ssh::write(&data_str, &coco_out_path, &sess).map_err(to_rv)?;
            }
        }
        CocoFileConnection::Local => {
            file_util::write(&coco_out_path, data_str)?;
        }
    }
    tracing::info!("exported coco labels to {coco_out_path:?}");
    Ok(coco_out_path)
}

pub fn read_coco(meta_data: &MetaData, coco_file: &CocoFile) -> RvResult<BboxSpecificData> {
    let coco_inpath = get_cocofilepath(meta_data, coco_file)?;
    match &coco_file.conn {
        CocoFileConnection::Local => {
            let s = file_util::read_to_string(&coco_inpath)?;
            let read_data: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
            tracing::info!("imported coco file from {coco_inpath:?}");
            read_data.convert_to_bboxdata(coco_file.clone())
        }
        CocoFileConnection::Ssh => {
            if let Some(ssh_cfg) = &meta_data.ssh_cfg {
                let sess = ssh::auth(ssh_cfg)?;
                let read_bytes = ssh::download(path_to_str(&coco_file.path)?, &sess)?;
                let s = String::from_utf8(read_bytes).map_err(to_rv)?;

                let read: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
                tracing::info!("imported coco file from {coco_inpath:?}");
                read.convert_to_bboxdata(coco_file.clone())
            } else {
                Err(rverr!("cannot read coco from ssh, ssh-cfg missing.",))
            }
        }
    }
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
    export_absolute: bool,
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
                println!("{e:?}");
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
    bbox_data.options.export_absolute = export_absolute;
    bbox_data.coco_file = CocoFile::default();
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
    if let Some(a) = annos {
        for bb in bbs {
            a.add_bb(bb, 0);
        }
    }
    (bbox_data, meta, test_export_path)
}

#[test]
fn test_coco_export() -> RvResult<()> {
    fn test(file_path: &Path, opened_folder: Option<&Path>, export_absolute: bool) -> RvResult<()> {
        let (bbox_data, meta, _) = make_data("json", &file_path, opened_folder, export_absolute);
        let coco_file = write_coco(&meta, bbox_data.clone())?;
        defer_file_removal!(&coco_file);
        let read = read_coco(
            &meta,
            &CocoFile {
                path: coco_file.clone(),
                conn: CocoFileConnection::Local,
            },
        )?;
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
    test(&file_path, None, true)?;
    let folder = Path::new("http://localhost:8000/some_path");
    let file = Path::new("http://localhost:8000/some_path/xyz.png");
    test(file, Some(folder), false)?;
    Ok(())
}

#[cfg(test)]
const TEST_DATA_FOLDER: &str = "resources/test_data/";

#[test]
fn test_coco_import() -> RvResult<()> {
    fn test(filename: &str, cat_ids: Vec<u32>, reference_bbs: &[(BB, &str)]) {
        let meta = MetaData {
            file_path: None,
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: Some(filename.to_string()),
            export_folder: Some(TEST_DATA_FOLDER.to_string()),
            is_loading_screen_active: None,
        };
        let read = read_coco(&meta, &CocoFile::default()).unwrap();
        assert_eq!(read.cat_ids(), &cat_ids);
        assert_eq!(read.labels(), &vec!["first label", "second label"]);
        for (bb, file_path) in reference_bbs {
            let annos = read.get_annos(file_path);
            println!("");
            println!("{file_path:?}");
            println!("{annos:?}");
            assert!(annos.unwrap().geos().contains(&GeoFig::BB(*bb)));
        }
    }

    let bb_im_ref_abs1 = [
        (
            BB::from_arr(&[1, 1, 5, 5]),
            "http://localhost:5000/%2Bnowhere.png",
        ),
        (
            BB::from_arr(&[11, 11, 4, 7]),
            "http://localhost:5000/%2Bnowhere.png",
        ),
        (
            BB::from_arr(&[1, 1, 5, 5]),
            "http://localhost:5000/%2Bnowhere2.png",
        ),
    ];
    let bb_im_ref_abs2 = [
        (BB::from_arr(&[1, 1, 5, 5]), "nowhere.png"),
        (BB::from_arr(&[11, 11, 4, 7]), "nowhere.png"),
        (BB::from_arr(&[1, 1, 5, 5]), "nowhere2.png"),
    ];
    let bb_im_ref_relative = [
        (BB::from_arr(&[10, 100, 50, 500]), "nowhere.png"),
        (BB::from_arr(&[91, 870, 15, 150]), "nowhere.png"),
        (BB::from_arr(&[10, 1, 50, 5]), "nowhere2.png"),
    ];
    test("catids_12", vec![1, 2], &bb_im_ref_abs1);
    test("catids_01", vec![0, 1], &bb_im_ref_abs2);
    test("catids_12_relative", vec![1, 2], &bb_im_ref_relative);
    Ok(())
}

#[test]
fn color_vs_str() {
    let colors = vec![[0, 0, 7], [4, 0, 101], [210, 9, 0]];
    let s = colors_to_string(&colors);
    let colors_back = string_to_colors(&s.unwrap()).unwrap();
    assert_eq!(colors, colors_back);
}
