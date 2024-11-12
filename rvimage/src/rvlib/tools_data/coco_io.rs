use std::{
    collections::HashMap,
    fmt::Debug,
    mem,
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
    vec,
};

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    cfg::{ExportPath, ExportPathConnection},
    file_util::{self, path_to_str, PathPair},
    image_util,
    meta_data::MetaData,
    result::trace_ok_warn,
    ssh,
    util::version_label,
    GeoFig, Polygon,
};
use rvimage_domain::{rle_image_to_bb, rle_to_mask, BbF, Canvas, Point, ShapeI, TPtF};
use rvimage_domain::{rverr, to_rv, RvError, RvResult};

use super::{
    annotations::InstanceAnnotations,
    brush_data::BrushAnnoMap,
    core::{new_random_colors, CocoSegmentation, ExportAsCoco},
    BboxToolData, BrushToolData, InstanceAnnotate, InstanceExportData, Rot90ToolData,
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
    bbox: [TPtF; 4],
    segmentation: Option<CocoSegmentation>,
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

fn insert_elt<A>(
    elt: A,
    annos: &mut HashMap<String, (Vec<A>, Vec<usize>, ShapeI)>,
    cat_idx: usize,
    n_rotations: u8,
    path_as_key: String,
    shape_coco: &ShapeI,
) where
    A: InstanceAnnotate,
{
    let geo = trace_ok_warn(elt.rot90_with_image_ntimes(shape_coco, n_rotations));
    if let Some(geo) = geo {
        if let Some(annos_of_image) = annos.get_mut(&path_as_key) {
            annos_of_image.0.push(geo);
            annos_of_image.1.push(cat_idx);
        } else {
            annos.insert(
                path_as_key,
                (
                    vec![geo],
                    vec![cat_idx],
                    ShapeI::new(shape_coco.w, shape_coco.h),
                ),
            );
        }
    }
}

fn instance_to_coco_anno<A>(
    inst_anno: &A,
    shape_im_unrotated: &ShapeI,
    n_rotations: u8,
    is_export_coords_absolute: bool,
) -> RvResult<([f64; 4], Option<CocoSegmentation>)>
where
    A: InstanceAnnotate,
{
    // to store data corresponding to the image on the disk, we need to invert the
    // applied rotations
    let n_rots_inverted = (4 - n_rotations) % 4;
    let shape_rotated = shape_im_unrotated.rot90_with_image_ntimes(n_rotations);
    let inst_anno = inst_anno
        .clone()
        .rot90_with_image_ntimes(&shape_rotated, n_rots_inverted)?;

    let bb = inst_anno.enclosing_bb();

    let segmentation = inst_anno.to_cocoseg(*shape_im_unrotated, is_export_coords_absolute)?;
    let (imw, imh) = if is_export_coords_absolute {
        (1.0, 1.0)
    } else {
        (shape_im_unrotated.w as TPtF, shape_im_unrotated.h as TPtF)
    };

    let bb_f = [bb.x / imw, bb.y / imh, bb.w / imw, bb.h / imh];
    if bb_f[1] * bb_f[2] < 1e-6 {
        tracing::warn!("annotation has no area {bb:?}.");
    }
    Ok((bb_f, segmentation))
}

struct WarnerCounting<'a> {
    n_warnings: usize,
    n_max_warnings: usize,
    suppressing: bool,
    suppress_str: &'a str,
}
impl<'a> WarnerCounting<'a> {
    fn new(n_max_warnings: usize, suppress_str: &'a str) -> Self {
        Self {
            n_warnings: 0,
            n_max_warnings,
            suppressing: false,
            suppress_str,
        }
    }
    fn warn_str<'b>(&mut self, msg: &'b str) -> Option<&'b str>
    where
        'a: 'b,
    {
        if self.n_warnings < self.n_max_warnings {
            self.n_warnings += 1;
            Some(msg)
        } else if !self.suppressing {
            self.suppressing = true;
            Some(self.suppress_str)
        } else {
            None
        }
    }
    fn warn(&mut self, msg: String) {
        if let Some(msg) = self.warn_str(msg.as_str()) {
            tracing::warn!(msg);
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CocoExportData {
    info: CocoInfo,
    images: Vec<CocoImage>,
    annotations: Vec<CocoAnnotation>,
    categories: Vec<CocoBboxCategory>,
}
impl CocoExportData {
    fn from_tools_data<T, A>(
        tools_data: T,
        rotation_data: Option<&Rot90ToolData>,
        prj_path: Option<&Path>,
    ) -> RvResult<Self>
    where
        T: ExportAsCoco<A>,
        A: InstanceAnnotate + 'static,
    {
        let (options, label_info, anno_map, coco_file) = tools_data.separate_data();
        let color_str = if let Some(s) = colors_to_string(label_info.colors()) {
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
        let export_data =
            InstanceExportData::from_tools_data(&options, label_info, coco_file, anno_map);

        type AnnotationMapValue<'a, A> = (&'a String, &'a (Vec<A>, Vec<usize>, ShapeI));
        let make_image_map =
            |(idx, (file_path, (_, _, shape))): (usize, AnnotationMapValue<A>)| CocoImage {
                id: idx as u32,
                width: shape.w,
                height: shape.h,
                file_name: file_path.clone(),
            };
        let images = export_data
            .annotations
            .iter()
            .enumerate()
            .map(make_image_map)
            .collect::<Vec<_>>();

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
        type AnnoType<'a, A> = (usize, (&'a String, &'a (Vec<A>, Vec<usize>, ShapeI)));
        let make_anno_map = |(image_idx, (file_path, (bbs, cat_idxs, shape))): AnnoType<A>| {
            let prj_path = if let Some(prj_path) = prj_path {
                prj_path
            } else {
                Path::new("")
            };
            let p = PathPair::new(file_path.clone(), prj_path);
            let p_abs = p.path_absolute();
            let shape = if Path::new(p_abs).exists() {
                let im = trace_ok_warn(image_util::read_image(file_path));
                if let Some(im) = im {
                    ShapeI::new(im.width(), im.height())
                } else {
                    *shape
                }
            } else {
                *shape
            };
            let n_rotations = get_n_rotations(rotation_data, file_path);
            bbs.iter()
                .zip(cat_idxs.iter())
                .flat_map(|(inst_anno, cat_idx): (&A, &usize)| {
                    trace_ok_warn(instance_to_coco_anno(
                        inst_anno,
                        &shape,
                        n_rotations,
                        options.is_export_absolute,
                    ))
                    .map(|(bb_f, segmentation)| {
                        box_id += 1;
                        CocoAnnotation {
                            id: box_id - 1,
                            image_id: image_idx as u32,
                            category_id: export_data.cat_ids[*cat_idx],
                            bbox: bb_f,
                            segmentation,
                            area: Some(bb_f[2] * bb_f[3]),
                        }
                    })
                })
                .collect::<Vec<_>>()
        };
        let annotations = export_data
            .annotations
            .iter()
            .enumerate()
            .flat_map(make_anno_map)
            .collect::<Vec<_>>();

        let coco_data = CocoExportData {
            info,
            images,
            annotations,
            categories,
        };
        Ok(coco_data)
    }

    fn convert_to_toolsdata(
        self,
        coco_file: ExportPath,
        rotation_data: Option<&Rot90ToolData>,
    ) -> RvResult<(BboxToolData, BrushToolData)> {
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

        let mut annotations_bbox: HashMap<String, (Vec<GeoFig>, Vec<usize>, ShapeI)> =
            HashMap::new();
        let mut annotations_brush: HashMap<String, (Vec<Canvas>, Vec<usize>, ShapeI)> =
            HashMap::new();

        let n_annotations = self.annotations.len();
        let mut warner = WarnerCounting::new(
            n_annotations / 10,
            "suppressing further warnings during coco import",
        );
        for coco_anno in self.annotations {
            let (file_path, w_coco, h_coco) = id_image_map[&coco_anno.image_id];

            let mut invalid_segmentation_exists = false;
            // The annotations in the coco files created by RV Image are stored
            // ignoring any orientation meta-data. Hence, if the image has been loaded
            // and rotated with RV Image we correct the rotation.
            let n_rotations = get_n_rotations(rotation_data, file_path);
            let shape_coco = ShapeI::new(w_coco, h_coco);

            let path_as_key = if file_path.starts_with("http") {
                file_util::url_encode(file_path)
            } else {
                file_path.to_string()
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

            let mut insert_geo = |geo| {
                insert_elt(
                    geo,
                    &mut annotations_bbox,
                    cat_idx,
                    n_rotations,
                    path_as_key.clone(),
                    &shape_coco,
                );
            };

            let bb = BbF::from(&bbox);
            match coco_anno.segmentation {
                Some(CocoSegmentation::Polygon(segmentation)) => {
                    let geo = if !segmentation.is_empty() {
                        if segmentation.len() > 1 {
                            return Err(rverr!(
                                "multiple polygons per box not supported. ignoring all but first."
                            ));
                        }
                        let n_points = segmentation[0].len();
                        let coco_data = &segmentation[0];

                        let poly_points = (0..n_points)
                            .step_by(2)
                            .flat_map(|idx| {
                                let p = Point {
                                    x: (coco_data[idx] * w_factor),
                                    y: (coco_data[idx + 1] * h_factor),
                                };
                                if bb.contains(p) {
                                    Some(p)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        let poly = Polygon::from_vec(poly_points);
                        match poly {
                            Ok(poly) => {
                                let encl_bb = poly.enclosing_bb();
                                if encl_bb.w * encl_bb.h < 1e-6 && bb.w * bb.h > 1e-6 {
                                    warner.warn(format!("polygon has no area. using bb. bb: {bb:?}, poly: {encl_bb:?}, file: {file_path}"));
                                    GeoFig::BB(bb)
                                } else {
                                    if !bb.all_corners_close(encl_bb) {
                                        let msg = format!("bounding box and polygon enclosing box do not match. using bb. bb: {bb:?}, poly: {encl_bb:?}, file: {file_path}");
                                        warner.warn(msg);
                                    }
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
                                        GeoFig::BB(bb)
                                    } else {
                                        GeoFig::Poly(poly)
                                    }
                                }
                            }
                            Err(_) => {
                                if n_points > 0 {
                                    invalid_segmentation_exists = true;
                                }
                                // polygon might be empty, we continue with the BB
                                GeoFig::BB(bb)
                            }
                        }
                    } else {
                        GeoFig::BB(bb)
                    };
                    insert_geo(geo);
                }
                Some(CocoSegmentation::Rle(rle)) => {
                    let bb = bb.into();
                    let rle_bb = rle_image_to_bb(&rle.counts, bb, ShapeI::from(rle.size))?;
                    let mask = rle_to_mask(&rle_bb, bb.w, bb.h);
                    let intensity = rle.intensity.unwrap_or(1.0);
                    let canvas = Canvas {
                        bb,
                        mask,
                        intensity,
                    };
                    insert_elt(
                        canvas,
                        &mut annotations_brush,
                        cat_idx,
                        n_rotations,
                        path_as_key,
                        &shape_coco,
                    );
                }
                _ => {
                    let geo = GeoFig::BB(bb);
                    insert_geo(geo);
                }
            }
            if invalid_segmentation_exists {
                warner.warn(format!("invalid segmentation in coco file {file_path}"));
            }
        }
        let bbox_data = BboxToolData::from_coco_export_data(InstanceExportData {
            labels: labels.clone(),
            colors: colors.clone(),
            cat_ids: cat_ids.clone(),
            annotations: annotations_bbox,
            coco_file: coco_file.clone(),
            is_export_absolute: false,
        })?;
        let brush_data = BrushToolData::from_coco_export_data(InstanceExportData {
            labels,
            colors,
            cat_ids,
            annotations: annotations_brush,
            coco_file,
            is_export_absolute: false,
        })?;
        Ok((bbox_data, brush_data))
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
        .as_ref()
        .map(|of| of.path_absolute())
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

pub fn to_per_file_crowd(brush_annotations_map: &mut BrushAnnoMap) {
    for (i, (filename, (annos, _))) in brush_annotations_map.iter_mut().enumerate() {
        if i % 10 == 0 {
            info!("export - image #{i} converting {filename} to per-image-crowd");
        }
        if let Some(max_catidx) = annos.cat_idxs().iter().max() {
            let mut canvas_idxes_of_cats = vec![vec![]; max_catidx + 1];
            for i in 0..(annos.elts().len()) {
                canvas_idxes_of_cats[annos.cat_idxs()[i]].push(i);
            }
            let mut merged_canvases = vec![None; max_catidx + 1];
            for (cat_idx, canvas_idxes) in canvas_idxes_of_cats.iter().enumerate() {
                let mut merged_canvas: Option<Canvas> = None;
                for canvas_idx in canvas_idxes.iter() {
                    let elt = &annos.elts()[*canvas_idx];
                    if let Some(merged_canvas) = &mut merged_canvas {
                        *merged_canvas = mem::take(merged_canvas).merge(elt);
                    } else {
                        merged_canvas = Some(elt.clone());
                    }
                }
                merged_canvases[cat_idx] = merged_canvas;
            }
            let mut cat_idxes = vec![];
            let elts = merged_canvases
                .into_iter()
                .enumerate()
                .flat_map(|(i, cvs)| cvs.map(|cvs| (i, cvs)))
                .map(|(i, cvs)| {
                    cat_idxes.push(i);
                    cvs
                })
                .collect();
            let n_elts = cat_idxes.len();
            let new_annos = trace_ok_warn(InstanceAnnotations::<Canvas>::new(
                elts,
                cat_idxes,
                vec![false; n_elts],
            ));
            if let Some(new_annos) = new_annos {
                *annos = new_annos;
            }
        }
    }
}

/// Serialize annotations in Coco format. Any orientations changes applied with the rotation tool
/// are reverted, since the rotation tool does not change the image file. Hence, the Coco file contains the annotation
/// relative to the image as it is found in memory ignoring any meta-data.
pub fn write_coco<T, A>(
    meta_data: &MetaData,
    tools_data: T,
    rotation_data: Option<&Rot90ToolData>,
    coco_file: ExportPath,
) -> RvResult<(PathBuf, JoinHandle<RvResult<()>>)>
where
    T: ExportAsCoco<A> + Send + 'static,
    A: InstanceAnnotate + 'static,
{
    let meta_data = meta_data.clone();
    let coco_out_path = get_cocofilepath(&meta_data, &coco_file)?;
    let coco_out_path_for_thr = coco_out_path.clone();
    let rotation_data = rotation_data.cloned();
    let conn = coco_file.conn.clone();
    let handle = thread::spawn(move || {
        let coco_data = CocoExportData::from_tools_data(
            tools_data,
            rotation_data.as_ref(),
            meta_data.prj_path(),
        )?;
        let data_str = serde_json::to_string(&coco_data).map_err(to_rv)?;
        conn.write(
            &data_str,
            &coco_out_path_for_thr,
            meta_data.ssh_cfg.as_ref(),
        )?;
        tracing::info!("exported coco labels to {coco_out_path_for_thr:?}");
        Ok(())
    });
    Ok((coco_out_path, handle))
}

/// Import annotations in Coco format. Any orientations changes applied with the rotation tool
/// to images that have annotations in the Coco file are applied to the annotations before importing. We expect, that
/// the Coco file contains the annotations relative to the image as it is found in memory ignoring any meta-data.
pub fn read_coco(
    meta_data: &MetaData,
    coco_file: &ExportPath,
    rotation_data: Option<&Rot90ToolData>,
) -> RvResult<(BboxToolData, BrushToolData)> {
    let coco_inpath = get_cocofilepath(meta_data, coco_file)?;
    match &coco_file.conn {
        ExportPathConnection::Local => {
            let s = file_util::read_to_string(&coco_inpath)?;
            let read_data: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
            tracing::info!("imported coco file from {coco_inpath:?}");
            read_data.convert_to_toolsdata(coco_file.clone(), rotation_data)
        }
        ExportPathConnection::Ssh => {
            if let Some(ssh_cfg) = &meta_data.ssh_cfg {
                tracing::info!("creating session based on {:?}", meta_data.ssh_cfg);
                let sess = ssh::auth(ssh_cfg)?;
                let read_bytes = ssh::download(path_to_str(&coco_file.path)?, &sess)?;
                let s = String::from_utf8(read_bytes).map_err(to_rv)?;

                let read: CocoExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
                tracing::info!("imported coco file from {coco_inpath:?}");
                read.convert_to_toolsdata(coco_file.clone(), rotation_data)
            } else {
                Err(rverr!("cannot read coco from ssh, ssh-cfg missing."))
            }
        }
    }
}

#[cfg(test)]
use {
    super::core::CocoRle,
    crate::{
        cfg::SshCfg,
        defer_file_removal,
        meta_data::{ConnectionData, MetaDataFlags},
    },
    file_util::DEFAULT_TMPDIR,
    rvimage_domain::{make_test_bbs, BbI},
    std::{fs, str::FromStr},
};
#[cfg(test)]
fn make_meta_data(opened_folder: Option<&Path>) -> (MetaData, PathBuf) {
    let opened_folder = if let Some(of) = opened_folder {
        PathPair::new(of.to_str().unwrap().to_string(), Path::new(""))
    } else {
        PathPair::new("xi".to_string(), Path::new(""))
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

    let test_export_path = DEFAULT_TMPDIR.join(format!("{}.json", opened_folder.path_absolute()));
    let mut meta = MetaData::from_filepath(
        test_export_path
            .with_extension("egal")
            .to_str()
            .unwrap()
            .to_string(),
        0,
        &Path::new("egal"),
    );
    meta.opened_folder = Some(opened_folder);
    meta.export_folder = Some(test_export_folder.to_str().unwrap().to_string());
    meta.connection_data = ConnectionData::Ssh(SshCfg::default());
    (meta, test_export_path)
}
#[cfg(test)]
fn make_data_brush(
    image_file: &Path,
    opened_folder: Option<&Path>,
    export_absolute: bool,
    n_boxes: Option<usize>,
) -> (BrushToolData, MetaData, PathBuf, ShapeI) {
    let shape = ShapeI::new(100, 40);
    let mut bbox_data = BrushToolData::default();
    bbox_data.options.core_options.is_export_absolute = export_absolute;
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
            let mut mask = vec![0; (bb.w * bb.h) as usize];
            mask[4] = 1;
            let c = Canvas {
                bb: bb.into(),
                mask,
                intensity: 0.5,
            };
            a.add_elt(c, 0);
        }
    }

    let (meta, test_export_path) = make_meta_data(opened_folder);
    (bbox_data, meta, test_export_path, shape)
}
#[cfg(test)]
pub fn make_data_bbox(
    image_file: &Path,
    opened_folder: Option<&Path>,
    export_absolute: bool,
    n_boxes: Option<usize>,
) -> (BboxToolData, MetaData, PathBuf, ShapeI) {
    let shape = ShapeI::new(20, 10);
    let mut bbox_data = BboxToolData::new();
    bbox_data.options.core_options.is_export_absolute = export_absolute;
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
    let (meta, test_export_path) = make_meta_data(opened_folder);
    (bbox_data, meta, test_export_path, shape)
}

#[cfg(test)]
fn is_image_duplicate_free(coco_data: &CocoExportData) -> bool {
    let mut image_ids = coco_data.images.iter().map(|i| i.id).collect::<Vec<_>>();
    image_ids.sort();
    let len_prev = image_ids.len();
    image_ids.dedup();
    image_ids.len() == len_prev
}

#[cfg(test)]
fn no_image_dups<P>(coco_file: P)
where
    P: AsRef<Path> + Debug,
{
    let s = file_util::read_to_string(&coco_file).unwrap();
    let read_raw: CocoExportData = serde_json::from_str(s.as_str()).unwrap();

    assert!(is_image_duplicate_free(&read_raw));
}
#[test]
fn test_coco_export() {
    fn assert_coco_eq<T, A>(data: T, read: T, coco_file: &PathBuf)
    where
        T: ExportAsCoco<A> + Send + 'static,
        A: InstanceAnnotate + 'static + Debug,
    {
        assert_eq!(data.label_info().cat_ids(), read.label_info().cat_ids());
        assert_eq!(data.label_info().labels(), read.label_info().labels());
        for (brush_anno, read_anno) in data.anno_iter().zip(read.anno_iter()) {
            let (name, (instance_annos, shape)) = brush_anno;
            let (read_name, (read_instance_annos, read_shape)) = read_anno;
            assert_eq!(instance_annos.cat_idxs(), read_instance_annos.cat_idxs());
            assert_eq!(
                instance_annos.elts().len(),
                read_instance_annos.elts().len()
            );
            for (i, (a, b)) in instance_annos
                .elts()
                .iter()
                .zip(read_instance_annos.elts().iter())
                .enumerate()
            {
                assert_eq!(a, b, "annos at index {} differ", i);
            }
            assert_eq!(name, read_name);
            assert_eq!(shape, read_shape);
        }
        no_image_dups(&coco_file);
    }
    fn write_read<T, A>(meta: &MetaData, tools_data: T) -> ((BboxToolData, BrushToolData), PathBuf)
    where
        T: ExportAsCoco<A> + Send + 'static,
        A: InstanceAnnotate + 'static,
    {
        let coco_file = tools_data.cocofile_conn();
        let (coco_file, handle) = write_coco(&meta, tools_data, None, coco_file).unwrap();
        handle.join().unwrap().unwrap();
        (
            read_coco(
                &meta,
                &ExportPath {
                    path: coco_file.clone(),
                    conn: ExportPathConnection::Local,
                },
                None,
            )
            .unwrap(),
            coco_file,
        )
    }
    fn test_br(file_path: &Path, opened_folder: Option<&Path>, export_absolute: bool) {
        let (brush_data, meta, _, _) =
            make_data_brush(&file_path, opened_folder, export_absolute, None);
        let ((_, read), coco_file) = write_read(&meta, brush_data.clone());
        defer_file_removal!(&coco_file);
        assert_coco_eq(brush_data, read, &coco_file);
    }
    fn test_bb(file_path: &Path, opened_folder: Option<&Path>, export_absolute: bool) {
        let (bbox_data, meta, _, _) =
            make_data_bbox(&file_path, opened_folder, export_absolute, None);
        let ((read, _), coco_file) = write_read(&meta, bbox_data.clone());
        defer_file_removal!(&coco_file);
        assert_coco_eq(bbox_data, read, &coco_file);
    }
    let tmpdir = &DEFAULT_TMPDIR;
    let file_path = tmpdir.join("test_image.png");
    test_br(&file_path, None, true);
    test_bb(&file_path, None, true);
    let folder = Path::new("http://localhost:8000/some_path");
    let file = Path::new("http://localhost:8000/some_path/xyz.png");
    test_br(file, Some(folder), false);
    test_bb(file, Some(folder), false);
}

#[cfg(test)]
const TEST_DATA_FOLDER: &str = "resources/test_data/";

#[test]
fn test_coco_import_export() {
    let meta = MetaData::new(
        None,
        None,
        ConnectionData::None,
        None,
        Some(PathPair::new("ohm_somefolder".to_string(), Path::new(""))),
        Some(TEST_DATA_FOLDER.to_string()),
        MetaDataFlags::default(),
        None,
    );
    let test_file_src = format!("{TEST_DATA_FOLDER}catids_12_coco_imwolab.json");
    let test_file = "tmp_coco.json";
    defer_file_removal!(&test_file);
    fs::copy(test_file_src, test_file).unwrap();
    let export_path = ExportPath {
        path: PathBuf::from_str(test_file).unwrap(),
        conn: ExportPathConnection::Local,
    };

    let (read, _) = read_coco(&meta, &export_path, None).unwrap();
    let (_, handle) = write_coco(&meta, read.clone(), None, export_path.clone()).unwrap();
    handle.join().unwrap().unwrap();
    no_image_dups(&read.coco_file.path);
    let (read, _) = read_coco(&meta, &export_path, None).unwrap();
    for anno in read.anno_iter() {
        let (_, (annos, _)) = anno;
        for a in annos.elts() {
            println!("{a:?}");
            assert!(a.enclosing_bb().w * a.enclosing_bb().h > 1e-3);
        }
    }
}

#[test]
fn test_coco_import() -> RvResult<()> {
    fn test(filename: &str, cat_ids: Vec<u32>, reference_bbs: &[(BbI, &str)]) {
        let meta = MetaData::new(
            None,
            None,
            ConnectionData::None,
            None,
            Some(PathPair::new(filename.to_string(), Path::new(""))),
            Some(TEST_DATA_FOLDER.to_string()),
            MetaDataFlags::default(),
            None,
        );
        let (read, _) = read_coco(&meta, &ExportPath::default(), None).unwrap();
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
    fn test<T, A>(
        coco_file: &PathBuf,
        bbox_specifics: T,
        meta_data: MetaData,
        shape: ShapeI,
        read_f: impl Fn(&MetaData, &ExportPath, Option<&Rot90ToolData>) -> T,
    ) where
        T: ExportAsCoco<A> + Send + 'static + Clone,
        A: InstanceAnnotate + 'static + Debug,
    {
        defer_file_removal!(&coco_file);
        let mut rotation_data = Rot90ToolData::default();
        let annos = rotation_data.get_annos_mut("some_path.png", shape);
        if let Some(annos) = annos {
            *annos = annos.increase();
        }
        let coco_file = bbox_specifics.cocofile_conn();
        let (out_path, handle) = write_coco(
            &meta_data,
            bbox_specifics.clone(),
            Some(&rotation_data),
            coco_file,
        )
        .unwrap();
        handle.join().unwrap().unwrap();
        println!("write to {out_path:?}");
        let out_path = ExportPath {
            path: out_path,
            conn: ExportPathConnection::Local,
        };
        let read = read_f(&meta_data, &out_path, Some(&rotation_data));

        for ((_, (anno_res, _)), (_, (anno_ref, _))) in
            bbox_specifics.anno_iter().zip(read.anno_iter())
        {
            for (read_elt, ref_elt) in anno_res.elts().iter().zip(anno_ref.elts().iter()) {
                assert_eq!(read_elt, ref_elt);
            }
        }
    }
    let (brush_specifics, meta_data, coco_file, shape) = make_data_brush(
        Path::new("some_path.png"),
        Some(Path::new("afolder")),
        false,
        None,
    );
    test(&coco_file, brush_specifics, meta_data, shape, |m, d, r| {
        read_coco(m, d, r).unwrap().1
    });
    let (bbox_specifics, meta_data, coco_file, shape) = make_data_bbox(
        Path::new("some_path.png"),
        Some(Path::new("afolder")),
        false,
        None,
    );
    test(&coco_file, bbox_specifics, meta_data, shape, |m, d, r| {
        read_coco(m, d, r).unwrap().0
    });
}

#[test]
fn test_serialize_rle() {
    let rle = CocoRle {
        counts: vec![1, 2, 3, 4],
        size: (5, 6),
        intensity: None,
    };
    let rle = CocoSegmentation::Rle(rle);
    let s = serde_json::to_string(&rle).unwrap();
    println!("{s}");
    let rle2: CocoSegmentation = serde_json::from_str(&s).unwrap();
    assert_eq!(format!("{rle:?}"), format!("{rle2:?}"));
    let poly = CocoSegmentation::Polygon(vec![vec![1.0, 2.0]]);
    let s = serde_json::to_string(&poly).unwrap();
    println!("{s}");
    let poly2: CocoSegmentation = serde_json::from_str(&s).unwrap();
    assert_eq!(format!("{poly:?}"), format!("{poly2:?}"));
}

#[test]
fn test_instance_to_coco() {
    let shape = ShapeI::new(2000, 2667);
    let bb = BbI::from_arr(&[1342, 1993, 8, 8]);
    let n_rot = 1;
    let canvas = Canvas {
        mask: vec![0; 64],
        bb,
        intensity: 0.5,
    };
    let coco_anno = instance_to_coco_anno(&canvas, &shape, n_rot, false);
    assert!(coco_anno.is_err());

    let shape_im = ShapeI::new(20, 40);
    let mut mask = vec![0; 4];
    mask[2] = 1;
    let canvas = Canvas {
        bb: BbI::from_arr(&[1, 1, 2, 2]),
        mask: mask.clone(),
        intensity: 0.5,
    };
    let n_rotations = 1;

    let (_, segmentation) = instance_to_coco_anno(&canvas, &shape_im, n_rotations, false).unwrap();

    let coco_seg = canvas
        .rot90_with_image_ntimes(
            &shape_im.rot90_with_image_ntimes(n_rotations),
            4 - n_rotations,
        )
        .unwrap()
        .to_cocoseg(shape_im, false)
        .unwrap();
    assert_ne!(coco_seg, None);
    assert_eq!(segmentation, coco_seg);
    let mut mask = vec![0; 4];
    mask[2] = 1;
    let geo = GeoFig::BB(BbF::from_arr(&[1.0, 1.0, 2.0, 8.0]));

    let n_rotations = 1;

    let (bb_rot, segmentation) = instance_to_coco_anno(&geo, &shape_im, n_rotations, true).unwrap();
    println!("{bb_rot:?}");
    let coco_seg = geo
        .rot90_with_image_ntimes(
            &shape_im.rot90_with_image_ntimes(n_rotations),
            4 - n_rotations,
        )
        .unwrap()
        .to_cocoseg(shape_im, true)
        .unwrap();
    assert_ne!(coco_seg, None);
    assert_eq!(segmentation, coco_seg);
}

#[test]
fn test_warner() {
    let suppress_msg = "no further warnings";
    let mut warner = WarnerCounting::new(3, suppress_msg);
    assert_eq!(warner.warn_str("a"), Some("a"));
    assert_eq!(warner.warn_str("a"), Some("a"));
    assert_eq!(warner.warn_str("b"), Some("b"));
    assert_eq!(warner.warn_str("a"), Some(suppress_msg));
    assert_eq!(warner.warn_str("a"), None);
}
