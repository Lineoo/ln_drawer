const GLOBAL_UPSCALE: f32 = 4.0;

pub fn svg_render(svg1: &[u8], upscale: f32) -> image::GrayAlphaImage {
    let tree = usvg::Tree::from_data(svg1, &usvg::Options::default()).unwrap();
    let upscale = upscale * GLOBAL_UPSCALE;
    let svg_size = tree.size();
    let mut mask_image = image::GrayImage::new(
        (svg_size.width() * upscale) as u32,
        (svg_size.height() * upscale) as u32,
    );
    let mut final_image = image::GrayAlphaImage::new(mask_image.width(), mask_image.height());
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
                xx: upscale,
                xy: 0.0,
                yx: 0.0,
                yy: upscale,
                x: 0.0,
                y: 0.0,
            }));
            mask.size(mask_image.width(), mask_image.height());
            mask.render_into(&mut mask_image, None);

            for (i, &luma) in mask_image.as_raw().iter().enumerate() {
                let slice = &mut *final_image;
                let src = luma as f32 / 255.;
                let dst = slice[i * 2 + 1] as f32 / 255.;
                let rst = src + dst * (1. - src);
                slice[i * 2 + 1] = (rst * 255.) as u8;
            }
        }
    }

    final_image
}
