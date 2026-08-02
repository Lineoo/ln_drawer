use cosmic_text::{Family, Metrics};
use glam::{IVec2, UVec2};
use ln_world::{Descriptor, Handle, World};
use palette::Srgba;

use crate::{
    layer::wrapper::{LayerDebugMessage, LayerWrapper},
    layout::transform::{Transform, TransformValue},
    lnwin::Lnwindow,
    measures::Rectangle,
    render::Render,
    widgets::{
        SetWidgetVisible,
        button::{ButtonClick, ToggleButton},
        panel::{Panel, SetPanelAnimation},
        renderer::text::Text,
    },
};

pub struct DebugPanel(pub Handle<ToggleButton>);
impl Descriptor for DebugPanel {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let button = self.0;

        let submenu = world.insert(Panel {
            rect: Rectangle::default(),
            visible: false,
            shadow: true,
        });

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();

        const HALF_INNER: UVec2 = UVec2::new(180, 120);
        const HALF_OUTER: UVec2 = UVec2::new(204, 144);

        let debug_text = world.insert(Text {
            text: "Hi there".into(),
            rect: Rectangle::new_half(IVec2::ZERO, HALF_INNER),
            metrics: Metrics::new(12.0, 18.0),
            family: Family::Monospace,
            color: Srgba::new(0, 0, 0, 1),
            upscale: lnwindow.window.scale_factor() as f32,
            order: 50,
            visible: false,
            outdated: true,
            canvas_outdated: false,
        });

        let submenu_transform = TransformValue::anchor(
            (1.0, 0.0),
            Rectangle::new_half(HALF_OUTER.as_ivec2() + IVec2::new(20, 0), HALF_OUTER),
        );

        let submenu_transform_start = TransformValue::anchor(
            (1.0, 0.0),
            Rectangle::new_half(
                HALF_OUTER.as_ivec2() / 4 + IVec2::new(20, 0),
                HALF_OUTER / 4,
            ),
        );

        world.insert(Transform {
            value: submenu_transform,
            source: button.untyped(),
            target: submenu.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::shrink(24, 24),
            source: submenu.untyped(),
            target: debug_text.untyped(),
        });

        world.observer(button, move |&ButtonClick, world| {
            let submenu = world.fetch(submenu).unwrap();
            let child2 = world.fetch(button).unwrap();
            world.queue_trigger(submenu.handle(), SetWidgetVisible(!submenu.visible));
            world.queue_trigger(debug_text, SetWidgetVisible(!submenu.visible));

            if !submenu.visible {
                world.queue_trigger(
                    submenu.handle(),
                    SetPanelAnimation {
                        src: submenu_transform_start.compute(child2.rect),
                        dst: submenu_transform.compute(child2.rect),
                        hidden_after_finished: false,
                    },
                );
            }

            let mut layer = world.single_fetch_mut::<LayerWrapper>().unwrap();
            layer.debug = !submenu.visible;
        });

        world.observer(
            world.single::<Render>().unwrap(),
            move |msg: &String, world| {
                let mut text = world.fetch_mut(debug_text).unwrap();
                if text.text != *msg {
                    text.text.clone_from(msg);
                    text.outdated = true;
                }
            },
        );

        world.observer(
            world.single::<LayerWrapper>().unwrap(),
            move |LayerDebugMessage(msg), world| {
                let render = world.single_fetch::<Render>().unwrap();
                if render.timestamp_poll {
                    return;
                }

                let mut text = world.fetch_mut(debug_text).unwrap();
                if text.text != *msg {
                    text.text.clone_from(msg);
                    text.outdated = true;
                }
            },
        );
    }
}
