use std::sync::Arc;

use cosmic_text::{Align, Attrs, Family, Metrics};
use ln_drawer::{
    layout::{transform::Transform, val::Val},
    lnwin::{Lnwin, LnwindowRect},
    measures::{Axis, IRect},
    theme::{Theme, ThemeVal},
    widgets::{
        button::{ButtonImage, ToggleButton},
        common::WidgetCommon,
        renderer::{
            rrect::RRect,
            svg::svg_render,
            text::{Text, TextAttr},
            vtor::{Vtor, logo},
        },
        slider::Slider,
    },
};
use ln_world::{Element, World};
use winit::event_loop::EventLoop;

const FONT_SANS: &str = "Source Han Sans CN";
const FONT_MONO: &str = "Noto Sans Mono";
const CARD_WIDTH: i32 = 364;
const CARD_HEIGHT: i32 = 136;

fn main() {
    env_logger::init();

    let lnwin = Lnwin {
        payload: Some(Arc::new(|world, common| {
            world.insert(WidgetShowcase { common });
        })),
        ..Default::default()
    };

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.run_app(lnwin).unwrap();
}

/// Widget gallery. Empty cards are kept as slots for upcoming widgets.
pub struct WidgetShowcase {
    pub common: WidgetCommon,
}

impl Element for WidgetShowcase {
    fn when_insert(&mut self, world: &World, this: ln_world::Handle<Self>) {
        world.scope("widget_showcase");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let root = world.single_fetch::<LnwindowRect>().0;
        let theme = world.single_fetch::<ThemeVal>();
        let metric_theme = world.single_fetch::<Theme>();
        let roundness = metric_theme.roundness;

        world.insert(RRect {
            common: self.common,
            rect: root,
            order: -200,
            color: theme.gray0,
            radius: 0.0,
            width: 0.0,
        });

        let title_rect = world.build(Transform::anchor(
            (0.0, 1.0),
            IRect::new(24, -58, 720, -20),
            root,
        ));
        world.insert(Text {
            text: "组件展示 / Widget Gallery".into(),
            rect: title_rect,
            attr: TextAttr {
                metrics: Metrics::new(28.0, 34.0),
                attrs: Attrs::new().family(Family::Name(FONT_SANS)),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order: 100,
            common: self.common,
            modulate: theme.distinct,
            outdated: false,
        });

        let card = |left: i32, down: i32, title: &str| {
            let outer = world.build(Transform::anchor(
                (0.0, 1.0),
                IRect::new(left, -down - CARD_HEIGHT, left + CARD_WIDTH, -down),
                root,
            ));
            world.insert(RRect {
                common: self.common,
                rect: outer,
                order: -10,
                color: theme.gray1,
                radius: roundness,
                width: 0.0,
            });
            let title_rect = world.build(Transform::anchor(
                (0.0, 1.0),
                IRect::new(16, -30, CARD_WIDTH - 16, -12),
                outer,
            ));
            world.insert(Text {
                text: title.into(),
                rect: title_rect,
                attr: TextAttr {
                    metrics: Metrics::new(12.0, 18.0),
                    attrs: Attrs::new().family(Family::Name(FONT_SANS)),
                    align: Align::Left,
                    extra_upscale: 1.0,
                },
                order: 100,
                common: self.common,
                modulate: theme.gray4,
                outdated: false,
            });
            outer
        };

        let local = |card, left: i32, top: i32, right: i32, bottom: i32| {
            world.build(Transform::anchor(
                (0.0, 1.0),
                IRect::new(left, -bottom, right, -top),
                card,
            ))
        };

        let text = |rect, text: &str, metrics, family, modulate, order| Text {
            text: text.into(),
            rect,
            attr: TextAttr {
                metrics,
                attrs: Attrs::new().family(Family::Name(family)),
                align: Align::Left,
                extra_upscale: 1.0,
            },
            order,
            common: self.common,
            modulate,
            outdated: false,
        };

        // Vtor: vector logo from vtor.rs.
        let vtor_card = card(24, 112, "Vtor");
        let logo_rect = local(vtor_card, 24, 30, 136, 126);
        // logo() is authored in a 64x64 SVG canvas; Vtor takes normalized paths.
        let logo_path = logo().transformed(
            &lyon::math::Transform::scale(1.0 / 64.0, -1.0 / 64.0)
                .then_translate(lyon::math::Vector::new(0.0, 1.0)),
        );
        world.insert(Vtor {
            common: self.common,
            rect: logo_rect,
            color: world.fetch(theme.theme).val,
            order: 10,
            path: logo_path,
            outdated: false,
        });
        let vtor_text = local(vtor_card, 150, 38, CARD_WIDTH - 16, 112);
        world.insert(text(
            vtor_text,
            "vtor.rs::logo()\n填充路径 + 三角化\nGPU 矢量绘制",
            Metrics::new(13.0, 22.0),
            FONT_MONO,
            theme.gray4,
            100,
        ));

        // Text: Latin and CJK samples.
        let text_card = card(412, 112, "Text");
        let text_rect = local(text_card, 24, 38, CARD_WIDTH - 16, 128);
        world.insert(text(
            text_rect,
            "文本渲染 / Text rendering\nSource Han Sans CN + fallback\nABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789\n天地玄黄，宇宙洪荒。",
            Metrics::new(14.0, 21.0),
            FONT_SANS,
            theme.distinct,
            100,
        ));

        // RRect: corner radius and theme colors.
        let rrect_card = card(24, 260, "RRect");
        let swatches = [theme.gray2, theme.gray3, theme.theme, theme.distinct];
        let radii = [4.0, 12.0, 22.0, 28.0];
        for (i, (&color, radius)) in swatches.iter().zip(radii).enumerate() {
            let left = 24 + i as i32 * 72;
            let rect = local(rrect_card, left, 42, left + 56, 98);
            world.insert(RRect {
                common: self.common,
                rect,
                order: 10,
                color,
                radius,
                width: 0.0,
            });
        }

        // Controls: interactive slider and toggle button.
        let controls_card = card(412, 260, "Sliders & ToggleButton");
        let slider_rect = local(controls_card, 24, 40, CARD_WIDTH - 24, 64);
        world.insert(Slider {
            common: self.common,
            value: world.insert(Val::new(0.42)),
            axis: Axis::Right,
            rect: slider_rect,
            pressed: false,
        });
        let button_rect = local(controls_card, 24, 78, 144, 118);
        world.insert(ToggleButton {
            common: self.common,
            rect: button_rect,
            theme: theme.button(),
            image: Some(ButtonImage::new(
                svg_render(include_bytes!("../res/interface/pen.svg")),
                theme.distinct,
                12,
            )),
            selected: world.insert(Val::new(true)),
            hovering: world.insert(Val::new(false)),
        });
        let button_text = local(controls_card, 160, 84, CARD_WIDTH - 16, 112);
        world.insert(text(
            button_text,
            "ToggleButton",
            Metrics::new(13.0, 20.0),
            FONT_SANS,
            theme.gray4,
            100,
        ));
    }
}
