use cosmic_text::{Attrs, Metrics};
use glam::{DVec2, I64Vec2};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    i18n::{tr, trp},
    layer::wrapper::LayerPage,
    layout::{
        echo::Echo,
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::Rectangle,
    theme::Theme,
    widgets::{
        button::{ButtonSelected, SetButtonSelected, ToggleButton, ToggleButtonTheme},
        common::WidgetCommon,
        container::{ContainerDeco, ContainerDecoDropdown, ContainerWithCamera},
        renderer::{rrect::RRect, text::Text},
    },
};

struct LayerChosen(u64);

pub struct LayerSelectionPanel {
    pub common: WidgetCommon,
}

impl Element for LayerSelectionPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let theme = world.single_fetch::<Theme>();

        let (panel, camera) = world.build(ContainerWithCamera {
            rect: Rectangle::default(),
            common: self.common,
            scroll: I64Vec2::ZERO,
            anchor: DVec2::new(0.0, 1.0),
        });
        let tab_layer_selection_deco = world.insert(ContainerDeco {
            container: panel,
            restriction: Rectangle::default(),
        });
        world.insert(ContainerDecoDropdown {
            height: None,
            container: panel,
            deco: tab_layer_selection_deco,
        });

        let doc_label = world.insert(Text {
            text: tr("settings.layer_selection.doc_label").into(),
            metrics: Metrics {
                font_size: 16.0,
                line_height: 18.0,
            },
            attrs: Attrs::new(),
            color: theme.distinct,
            camera: Some(camera),
            visible: false,
            ..Default::default()
        });

        let doc_label_div = world.insert(RRect {
            rect: Rectangle::default(),
            order: 0,
            color: theme.gray2,
            radius: 0.0,
            width: 0.0,
            enabled: false,
            camera,
        });

        let layer_label = world.insert(Text {
            text: tr("settings.layer_selection.layer_label").into(),
            metrics: Metrics {
                font_size: 14.0,
                line_height: 16.0,
            },
            attrs: Attrs::new(),
            color: theme.distinct,
            camera: Some(camera),
            visible: false,
            ..Default::default()
        });

        Transfer::new(world, this, panel)
            .set_widget_rectangle()
            .set_widget_visible();

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
                common: WidgetCommon {
                    camera,
                    visible: self.common.visible,
                },
                rect: Rectangle::default(),
                theme: ToggleButtonTheme {
                    idle_color: theme.gray1,
                    hover_color: theme.gray2,
                    press_color: theme.gray3,
                    selected_color: theme.gray3,
                },
                image: None,
                selected: i == 0,
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
                color: theme.distinct,
                camera: Some(camera),
                visible: false,
                ..Default::default()
            });

            world.observer(layer0_button, move |&ButtonSelected(val), world| {
                if val {
                    let mut wrapper = world.single_fetch_mut::<LayerPage>();
                    wrapper.set_page(i as u64);
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

            world.dependency(layer0_button, panel);
            world.dependency(layer0_name, panel);
        }

        world.dependency(doc_label, panel);
        world.dependency(doc_label_div, panel);
        world.dependency(layer_label, panel);
        world.dependency(layers_node, panel);
    }
}
