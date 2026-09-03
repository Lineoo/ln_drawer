use cosmic_text::{Attrs, Metrics};
use ln_world::{Handle, HandleGeneric, World};

use crate::{
    i18n::{tr, trp},
    layer::{stream::ThreadInput, wrapper::LayerWrapper},
    layout::transform::{Transform, TransformEdge, TransformValue},
    measures::Rectangle,
    theme::Theme,
    widgets::{
        button::{ButtonSelected, SetButtonSelected, ToggleButton, ToggleButtonTheme},
        container::Container,
        echo::Echo,
        renderer::{rrect::RRect, text::Text},
    },
};

struct LayerChosen(u64);

pub fn layer_selection(world: &World, panel: Handle<Container>) {
    let theme = world.single_fetch::<Theme>().unwrap();

    let doc_label = world.insert(Text {
        text: tr("settings.layer_selection.doc_label").into(),
        metrics: Metrics {
            font_size: 16.0,
            line_height: 18.0,
        },
        attrs: Attrs::new(),
        color: theme.symbolic_color,
        ..Default::default()
    });

    let doc_label_div = world.insert(RRect {
        rect: Rectangle::default(),
        order: 0,
        color: theme.secondary_color,
        radius: 0.0,
        width: 0.0,
        enabled: true,
    });

    let layer_label = world.insert(Text {
        text: tr("settings.layer_selection.layer_label").into(),
        metrics: Metrics {
            font_size: 14.0,
            line_height: 16.0,
        },
        attrs: Attrs::new(),
        color: theme.symbolic_color,
        ..Default::default()
    });

    world.insert(Transform {
        value: TransformValue::anchor((0.0, 1.0), Rectangle::new(28, -22 - 18, 28 + 32, -22)),
        source: panel.untyped(),
        target: doc_label.untyped(),
    });

    world.insert(Transform {
        value: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 0,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -61,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: 0,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: -59,
            },
        },
        source: panel.untyped(),
        target: doc_label_div.untyped(),
    });

    world.insert(Transform {
        value: TransformValue::anchor((0.0, 1.0), Rectangle::new(28, -90, 28 + 32, -90 + 16)),
        source: panel.untyped(),
        target: layer_label.untyped(),
    });

    let layers_node = world.insert(());
    for i in 0..3i32 {
        let layer0_button = world.insert(ToggleButton {
            rect: Rectangle::default(),
            theme: ToggleButtonTheme {
                idle_color: theme.primary_color,
                hover_color: theme.secondary_color,
                press_color: theme.highlight_color,
                selected_color: theme.highlight_color,
            },
            image: None,
            selected: i == 0,
            visible: true,
            hovering: false,
        });

        let layer0_name = world.insert(Text {
            text: trp(
                "settings.layer_selection.layer_name",
                &[("index", &i.to_string()[..])],
            ),
            metrics: Metrics {
                font_size: 14.0,
                line_height: 16.0,
            },
            attrs: Attrs::new(),
            color: theme.symbolic_color,
            ..Default::default()
        });

        world.observer(layer0_button, move |&ButtonSelected(val), world| {
            if val {
                let wrapper = world.single_fetch::<LayerWrapper>().unwrap();
                wrapper
                    .thread_tx
                    .send(ThreadInput::SetPage(i as u64))
                    .unwrap();
                world.queue_trigger(layers_node, LayerChosen(i as u64));
            }
        });

        world.observer(layers_node, move |&LayerChosen(j), world| {
            world.queue_trigger(layer0_button, SetButtonSelected(i as u64 == j));
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 10,
                },
                down: TransformEdge {
                    anchor: 1.0,
                    offset: -100 - 48 * (i + 1),
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: -10,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -100 - 48 * i,
                },
            },
            source: panel.untyped(),
            target: layer0_button.untyped(),
        });

        Echo::new(world, layer0_button)
            .widget_rectangle()
            .widget_visible();

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 18,
                },
                down: TransformEdge {
                    anchor: 0.0,
                    offset: 16,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: -18,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -16,
                },
            },
            source: layer0_button.untyped(),
            target: layer0_name.untyped(),
        });
    }
}
