use glam::{IVec2, UVec2};

use crate::{layout::transform::TransformValue, measures::Rectangle, widgets::button::ButtonImage};

const GLOBAL_UPSCALE: f32 = 4.0;

pub fn svg_render(svg1: &[u8], size: u32) -> ButtonImage {
    let tree = usvg::Tree::from_data(svg1, &usvg::Options::default()).unwrap();
    let svg_size = tree.size();
    let real_width = (svg_size.width() * GLOBAL_UPSCALE).ceil() as usize;
    let real_height = (svg_size.height() * GLOBAL_UPSCALE).ceil() as usize;
    let mut mask_image = vec![0u8; real_width * real_height];
    let mut final_image = vec![0u8; 4 * real_width * real_height];
    for node in tree.root().children() {
        if let usvg::Node::Path(path) = node {
            let points = path
                .data()
                .points()
                .iter()
                .map(|p| zeno::Point { x: p.x, y: p.y })
                .collect::<Vec<_>>();
            let verbs = path
                .data()
                .verbs()
                .iter()
                .map(|p| match p {
                    usvg::tiny_skia_path::PathVerb::Move => zeno::Verb::MoveTo,
                    usvg::tiny_skia_path::PathVerb::Line => zeno::Verb::LineTo,
                    usvg::tiny_skia_path::PathVerb::Quad => zeno::Verb::QuadTo,
                    usvg::tiny_skia_path::PathVerb::Cubic => zeno::Verb::CurveTo,
                    usvg::tiny_skia_path::PathVerb::Close => zeno::Verb::Close,
                })
                .collect::<Vec<_>>();
            let mut mask = zeno::Mask::new((&points[..], &verbs[..]));
            if let Some(stroke) = path.stroke() {
                let mut zeno_stroke = zeno::Stroke::new(stroke.width().get());
                zeno_stroke.join(match stroke.linejoin() {
                    usvg::LineJoin::Miter => zeno::Join::Miter,
                    usvg::LineJoin::MiterClip => zeno::Join::Miter,
                    usvg::LineJoin::Round => zeno::Join::Round,
                    usvg::LineJoin::Bevel => zeno::Join::Bevel,
                });
                zeno_stroke.cap(match stroke.linecap() {
                    usvg::LineCap::Butt => zeno::Cap::Butt,
                    usvg::LineCap::Round => zeno::Cap::Round,
                    usvg::LineCap::Square => zeno::Cap::Square,
                });
                if let Some(dasharray) = stroke.dasharray() {
                    zeno_stroke.dash(dasharray, stroke.dashoffset());
                }
                mask.style(zeno_stroke);
            }
            mask.transform(Some(zeno::Transform {
                xx: GLOBAL_UPSCALE,
                xy: 0.0,
                yx: 0.0,
                yy: GLOBAL_UPSCALE,
                x: 0.0,
                y: 0.0,
            }));
            mask.size(real_width as u32, real_height as u32);
            mask.render_into(&mut mask_image, None);

            for (i, &luma) in mask_image.iter().enumerate() {
                let slice = &mut *final_image;
                let src = luma as f32 / 255.;
                let dst = slice[i * 4 + 3] as f32 / 255.;
                let rst = src + dst * (1. - src);
                slice[i * 4] = 255;
                slice[i * 4 + 1] = 255;
                slice[i * 4 + 2] = 255;
                slice[i * 4 + 3] = (rst * 255.) as u8;
            }
        }
    }

    ButtonImage {
        transform: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(size)),
        ),
        bytes: final_image,
        bytes_width: real_width as u32,
        bytes_height: real_height as u32,
    }
}
