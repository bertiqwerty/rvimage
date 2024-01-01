use std::{
    collections::HashMap,
    fmt::Debug,
    iter,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    cfg::{ExportPath, ExportPathConnection},
    domain::{BbF, Point, ShapeI, TPtF},
    file_util::{self, path_to_str, MetaData},
    result::{to_rv, RvError, RvResult},
    rverr, ssh,
    util::version_label,
    GeoFig, Polygon,
};

use super::{core::new_random_colors, BboxExportData, BboxSpecificData, Rot90ToolData};

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
    bbox: [TPtF; 4],
    segmentation: Option<Vec<Vec<TPtF>>>,
    area: Option<TPtF>,
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

fn get_n_rotations(rotation_data: Option<&Rot90ToolData>, file_path: &str) -> u8 {
    rotation_data
        .and_then(|d| d.get_annos(file_path))
        .map(|n_rot| n_rot.to_num())
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Debug)]
struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}
impl CocoExportData {
    fn from_bboxdata(
        bbox_specifics: BboxSpecificData,
        rotation_data: Option<&Rot90ToolData>,
    ) -> RvResult<Self> {
        let color_str = if let Some(s) = colors_to_string(bbox_specifics.label_info.colors()) {
            format!(", {s}")
        } else {
            "".to_string()
        };
        let info_str = format!(
            "created with RV Image {}, https://github.com/bertiqwerty/rvimage{color_str}",
            version_label()
        );
        let info = CocoInfo {
            description: info_str,
        };
        let export_data = BboxExportData::from_bbox_data(bbox_specifics);

        type AnnotationMapValue<'a> = (&'a String, &'a (Vec<GeoFig>, Vec<usize>, ShapeI));
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
        type AnnoType<'a> = (usize, (&'a String, &'a (Vec<GeoFig>, Vec<usize>, ShapeI)));
        let make_anno_map = |(image_idx, (file_path, (bbs, cat_idxs, shape))): AnnoType| {
            bbs.iter()
                .zip(cat_idxs.iter())
                .map(|(geo, cat_idx): (&GeoFig, &usize)| {
                    let n_rotations = get_n_rotations(rotation_data, file_path);
                    // to store data corresponding to the image on the disk, we need to invert the
                    // applied rotations
                    let n_rots_inverted = (4 - n_rotations) % 4;
                    let geo = geo.clone().rot90_with_image_ntimes(shape, n_rots_inverted);

                    let bb = geo.enclosing_bb();
                    let (imw, imh) = if export_data.is_export_absolute {
                        (1.0, 1.0)
                    } else {
                        (shape.w as TPtF, shape.h as TPtF)
                    };
                    let segmentation = geo.points_normalized(imw, imh);
                    let segmentation = segmentation
                        .iter()
                        .flat_map(|p| iter::once(p.x).chain(iter::once(p.y)))
                        .collect::<Vec<_>>();
                    let bb_f = [bb.x / imw, bb.y / imh, bb.w / imw, bb.h / imh];
                    box_id += 1;
                    CocoAnnotation {
                        id: box_id - 1,
                        image_id: image_idx as u32,
                        category_id: export_data.cat_ids[*cat_idx],
                        bbox: bb_f,
                        segmentation: Some(vec![segmentation]),
                        area: Some(bb.h * bb.w),
                    }
                })
                .collect::<Vec<_>>()
        };
        let annotations = export_data
            .annotations
            .iter()
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

    fn convert_to_bboxdata(
        self,
        coco_file: ExportPath,
        rotation_data: Option<&Rot90ToolData>,
    ) -> RvResult<BboxSpecificData> {
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

        let mut annotations: HashMap<String, (Vec<GeoFig>, Vec<usize>, ShapeI)> = HashMap::new();
        for coco_anno in self.annotations {
            let (file_path, w_coco, h_coco) = id_image_map[&coco_anno.image_id];

            let coords_absolute = coco_anno.bbox.iter().any(|x| *x > 1.0);
            let (w_factor, h_factor) = if coords_absolute {
                (1.0, 1.0)
            } else {
                (w_coco as f64, h_coco as f64)
            };
            let bbox = [
                (w_factor * coco_anno.bbox[0]),
                (h_factor * coco_anno.bbox[1]),
                (w_factor * coco_anno.bbox[2]),
                (h_factor * coco_anno.bbox[3]),
            ];
            let bb = BbF::from(&bbox);
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
                                x: (coco_data[idx] * w_factor),
                                y: (coco_data[idx + 1] * h_factor),
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

            // The annotations in the coco files created by RV Image are stored
            // ignoring any orientation meta-data. Hence, if the image has been loaded
            // and rotated with RV Image we correct the rotation.
            let n_rotations = get_n_rotations(rotation_data, file_path);
            let shape_coco = ShapeI::new(w_coco, h_coco);
            let shape_rotated = shape_coco.rot90_with_image_ntimes(n_rotations);
            let geo = geo.rot90_with_image_ntimes(&shape_rotated, n_rotations);

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
            let k = if file_path.starts_with("http") {
                file_util::url_encode(file_path)
            } else {
                file_path.to_string()
            };
            if let Some(annos_of_image) = annotations.get_mut(&k) {
                annos_of_image.0.push(geo);
                annos_of_image.1.push(cat_idx);
            } else {
                annotations.insert(k, (vec![geo], vec![cat_idx], ShapeI::new(w_coco, h_coco)));
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
fn get_cocofilepath(meta_data: &MetaData, coco_file: &ExportPath) -> RvResult<PathBuf> {
    if path_to_str(&coco_file.path)?.is_empty() {
        meta_data_to_coco_path(meta_data)
    } else {
        Ok(coco_file.path.clone())
    }
}

/// Serialize annotations in Coco format. Any orientations changes applied with the rotation tool
/// are reverted, since the rotation tool does not change the image file. Hence, the Coco file contains the annotation
/// relative to the image as it is found in memory ignoring any meta-data.
pub fn write_coco(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
    rotation_data: Option<&Rot90ToolData>,
) -> RvResult<PathBuf> {
    let coco_out_path = get_cocofilepath(meta_data, &bbox_specifics.coco_file)?;
    let conn = bbox_specifics.coco_file.conn.clone();
    let coco_data = CocoExportData::from_bboxdata(bbox_specifics, rotation_data)?;
    let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
    conn.write(&data_str, &coco_out_path, meta_data.ssh_cfg.as_ref())?;
    tracing::info!("exported coco labels to {coco_out_path:?}");
    Ok(coco_out_path)
}

/// Import annotations in Coco format. Any orientations changes applied with the rotation tool
/// to images that have annotations in the Coco file are applied to the annotations before importing. We expect, that
/// the Coco file contains the annotations relative to the image as it is found in memory ignoring any meta-data.
pub fn read_coco(
    meta_data: &MetaData,
    coco_file: &ExportPath,
    rotation_data: Option<&Rot90ToolData>,
) -> RvResult<BboxSpecificData> {
    let coco_inpath = get_cocofilepath(meta_data, coco_file)?;
    match &coco_file.conn {
        ExportPathConnection::Local => {
            let s = file_util::read_to_string(&coco_inpath)?;
            let read_data: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
            tracing::info!("imported coco file from {coco_inpath:?}");
            read_data.convert_to_bboxdata(coco_file.clone(), rotation_data)
        }
        ExportPathConnection::Ssh => {
            if let Some(ssh_cfg) = &meta_data.ssh_cfg {
                let sess = ssh::auth(ssh_cfg)?;
                let read_bytes = ssh::download(path_to_str(&coco_file.path)?, &sess)?;
                let s = String::from_utf8(read_bytes).map_err(to_rv)?;

                let read: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
                tracing::info!("imported coco file from {coco_inpath:?}");
                read.convert_to_bboxdata(coco_file.clone(), rotation_data)
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
        domain::{make_test_bbs, BbI},
    },
    file_util::{ConnectionData, DEFAULT_TMPDIR},
    std::{fs, str::FromStr},
};

#[cfg(test)]
pub fn make_data(
    image_file: &Path,
    opened_folder: Option<&Path>,
    export_absolute: bool,
    n_boxes: Option<usize>,
) -> (BboxSpecificData, MetaData, PathBuf, ShapeI) {
    let shape = ShapeI::new(20, 10);
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

    let test_export_path = DEFAULT_TMPDIR.join(format!("{}.json", opened_folder));
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
    bbox_data.coco_file = ExportPath::default();
    bbox_data
        .label_info
        .push("x".to_string(), None, None)
        .unwrap();

    bbox_data
        .label_info
        .remove_catidx(0, &mut bbox_data.annotations_map);

    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    if let Some(n) = n_boxes {
        bbs = bbs[0..n].to_vec();
    }

    let annos = bbox_data.get_annos_mut(image_file.as_os_str().to_str().unwrap(), shape);
    if let Some(a) = annos {
        for bb in bbs {
            a.add_bb(bb, 0);
        }
    }
    (bbox_data, meta, test_export_path, shape)
}

#[test]
fn test_coco_export() -> RvResult<()> {
    fn test(file_path: &Path, opened_folder: Option<&Path>, export_absolute: bool) -> RvResult<()> {
        let (bbox_data, meta, _, _) = make_data(&file_path, opened_folder, export_absolute, None);
        let coco_file = write_coco(&meta, bbox_data.clone(), None)?;
        defer_file_removal!(&coco_file);
        let read = read_coco(
            &meta,
            &ExportPath {
                path: coco_file.clone(),
                conn: ExportPathConnection::Local,
            },
            None,
        )?;
        assert_eq!(bbox_data.label_info.cat_ids(), read.label_info.cat_ids());
        assert_eq!(bbox_data.label_info.labels(), read.label_info.labels());
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
    fn test(filename: &str, cat_ids: Vec<u32>, reference_bbs: &[(BbI, &str)]) {
        let meta = MetaData {
            file_path: None,
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: Some(filename.to_string()),
            export_folder: Some(TEST_DATA_FOLDER.to_string()),
            is_loading_screen_active: None,
            is_file_list_empty: None,
        };
        let read = read_coco(&meta, &ExportPath::default(), None).unwrap();
        assert_eq!(read.label_info.cat_ids(), &cat_ids);
        assert_eq!(
            read.label_info.labels(),
            &vec!["first label", "second label"]
        );
        for (bb, file_path) in reference_bbs {
            let annos = read.get_annos(file_path);
            println!("");
            println!("{file_path:?}");
            println!("{annos:?}");
            assert!(annos.unwrap().elts().contains(&GeoFig::BB((*bb).into())));
        }
    }

    let bb_im_ref_abs1 = [
        (
            BbI::from_arr(&[1, 1, 5, 5]),
            "http://localhost:5000/%2Bnowhere.png",
        ),
        (
            BbI::from_arr(&[11, 11, 4, 7]),
            "http://localhost:5000/%2Bnowhere.png",
        ),
        (
            BbI::from_arr(&[1, 1, 5, 5]),
            "http://localhost:5000/%2Bnowhere2.png",
        ),
    ];
    let bb_im_ref_abs2 = [
        (BbI::from_arr(&[1, 1, 5, 5]), "nowhere.png"),
        (BbI::from_arr(&[11, 11, 4, 7]), "nowhere.png"),
        (BbI::from_arr(&[1, 1, 5, 5]), "nowhere2.png"),
    ];
    let bb_im_ref_relative = [
        (BbI::from_arr(&[10, 100, 50, 500]), "nowhere.png"),
        (BbI::from_arr(&[91, 870, 15, 150]), "nowhere.png"),
        (BbI::from_arr(&[10, 1, 50, 5]), "nowhere2.png"),
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

#[test]
fn test_rotation_export_import() {
    let (bbox_specifics, meta_data, coco_file, shape) = make_data(
        Path::new("some_path.png"),
        Some(Path::new("afolder")),
        false,
        None,
    );
    defer_file_removal!(&coco_file);
    let mut rotation_data = Rot90ToolData::default();
    let annos = rotation_data.get_annos_mut("some_path.png", shape);
    if let Some(annos) = annos {
        *annos = annos.increase();
    }
    let out_path = write_coco(&meta_data, bbox_specifics.clone(), Some(&rotation_data)).unwrap();
    println!("write to {out_path:?}");
    let out_path = ExportPath {
        path: out_path,
        conn: ExportPathConnection::Local,
    };
    let read = read_coco(&meta_data, &out_path, Some(&rotation_data)).unwrap();
    for k in read.annotations_map.keys() {
        let (read_anno, _) = &read.annotations_map[k];
        let (ref_anno, _) = &bbox_specifics.annotations_map[k];
        for (read_elt, ref_elt) in read_anno.elts().iter().zip(ref_anno.elts().iter()) {
            assert_eq!(read_elt, ref_elt);
        }
    }
    assert_eq!(read.annotations_map, bbox_specifics.annotations_map);
}
