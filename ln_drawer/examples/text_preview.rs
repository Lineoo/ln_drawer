use std::sync::Arc;

use cosmic_text::{Align, Attrs, Family, Metrics};
use ln_drawer::{
    layout::transform::{Transform, TransformValue},
    lnwin::{Lnwin, LnwindowRect},
    measures::IRect,
    theme::ThemeVal,
    widgets::{
        common::WidgetCommon,
        renderer::text::{Text, TextAttr},
    },
};
use ln_world::{Element, Handle, World};
use winit::event_loop::EventLoop;

fn main() {
    env_logger::init();

    let lnwin = Lnwin {
        payload: Some(Arc::new(|world, common| {
            world.insert(TextPreview { common });
        })),
        ..Default::default()
    };

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.run_app(lnwin).unwrap();
}

pub struct TextPreview {
    pub common: WidgetCommon,
}

impl Element for TextPreview {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("text_preview");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let root = world.single_fetch::<LnwindowRect>().0;
        let theme = world.single_fetch::<ThemeVal>();

        let row = |top: i32, height: i32| {
            world.build(Transform::anchor(
                (0.0, 1.0),
                IRect::new(40, -top - height, 760, -top),
                root,
            ))
        };

        let title = row(40, 60);
        let latin = row(120, 100);
        let cjk = row(250, 100);
        let left = world.build(Transform::new(
            TransformValue::anchor((0.0, 1.0), IRect::new(40, -460, 260, -380)),
            root,
        ));
        let center = world.build(Transform::new(
            TransformValue::anchor((0.0, 1.0), IRect::new(280, -460, 500, -380)),
            root,
        ));
        let right = world.build(Transform::new(
            TransformValue::anchor((0.0, 1.0), IRect::new(520, -460, 760, -380)),
            root,
        ));
        let small = row(490, 60);

        world.insert(Text {
            text: "GPU 文本渲染 / GPU Text".into(),
            rect: title,
            attr: TextAttr {
                metrics: Metrics::new(32.0, 38.0),
                attrs: Attrs::new().family(Family::SansSerif),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order: 10,
            common: self.common,
            modulate: theme.theme,
            outdated: false,
        });

        world.insert(Text {
            text: "The quick brown fox jumps over the lazy dog.\nABCDEFGHIJKLMNOPQRSTUVWXYZ\n0123456789 !@#$%^&*()".into(),
            rect: latin,
            attr: TextAttr {
                metrics: Metrics::new(18.0, 24.0),
                attrs: Attrs::new().family(Family::Monospace),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order: 10,
            common: self.common,
            modulate: theme.gray0,
            outdated: false,
        });

        world.insert(Text {
            text: "永和九年，岁在癸丑，暮春之初。\n中文字体测试：天地玄黄，宇宙洪荒。\n日本語のテキスト / 한국어 텍스트".into(),
            rect: cjk,
            attr: TextAttr {
                metrics: Metrics::new(18.0, 26.0),
                attrs: Attrs::new().family(Family::SansSerif),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order: 10,
            common: self.common,
            modulate: theme.gray0,
            outdated: false,
        });

        for (rect, align) in [
            (left, Align::Left),
            (center, Align::Center),
            (right, Align::Right),
        ] {
            world.insert(Text {
                text: format!("对齐 / {align:?}\n第二行 line two").into(),
                rect,
                attr: TextAttr {
                    metrics: Metrics::new(14.0, 20.0),
                    attrs: Attrs::new(),
                    align,
                    extra_upscale: 1.0,
                },
                order: 10,
                common: self.common,
                modulate: theme.theme,
                outdated: false,
            });
        }

        world.insert(Text {
            text: "small text 12px：0123456789 中文小字".into(),
            rect: small,
            attr: TextAttr {
                metrics: Metrics::new(12.0, 18.0),
                attrs: Attrs::new(),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order: 10,
            common: self.common,
            modulate: theme.gray0,
            outdated: false,
        });
    }
}
