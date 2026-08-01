use cosmic_text::Metrics;
use glam::{IVec2, UVec2};
use ln_world::{Descriptor, Handle, World};
use palette::Srgba;

use crate::{
    layer::wrapper::{LayerDebugMessage, LayerWrapper},
    layout::transform::{Transform, TransformValue},
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{
        Render,
        text::{Text, TextChanged},
    },
    widgets::{
        WidgetClick, WidgetEnabled,
        button::Button,
        panel::{Panel, PanelAnimation},
    },
};

pub struct DebugPanel(pub Handle<Button>);
impl Descriptor for DebugPanel {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let button = self.0;

        let submenu = world.insert(Panel {
            rect: Rectangle::default(),
            visible: false,
        });

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();

        const HALF_INNER: UVec2 = UVec2::new(180, 120);
        const HALF_OUTER: UVec2 = UVec2::new(204, 144);

        let debug_text = world.insert(Text {
            text: "Hi there".into(),
            rect: Rectangle::new_half(IVec2::ZERO, HALF_INNER),
            metrics: Metrics::new(12.0, 18.0),
            color: Srgba::new(0, 0, 0, 1),
            upscale: lnwindow.window.scale_factor() as f32,
            order: 1,
            visible: false,
            outdated: true,
        });

        world.queue(move |world| {
            Panel::receive_event(submenu, world);
            Panel::create_renderer(submenu, world);
            Panel::create_interact(submenu, world);
            world
                .fetch_mut(debug_text)
                .unwrap()
                .bind_render(world, debug_text);
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

        world.observer(button, move |&WidgetClick, world| {
            let submenu = world.fetch(submenu).unwrap();
            let child2 = world.fetch(button).unwrap();
            world.queue_trigger(submenu.handle(), WidgetEnabled(!submenu.visible));
            world.queue_trigger(debug_text, WidgetEnabled(!submenu.visible));

            if !submenu.visible {
                world.queue_trigger(
                    submenu.handle(),
                    PanelAnimation {
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
                    world.queue_trigger(debug_text, TextChanged);
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
                    world.queue_trigger(debug_text, TextChanged);
                }
            },
        );
    }
}
