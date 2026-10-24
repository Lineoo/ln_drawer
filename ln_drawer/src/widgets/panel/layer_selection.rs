use cosmic_text::{Attrs, Metrics};
use ln_world::{Element, Handle, World};

use crate::{
    i18n::{tr, trp},
    layer::wrapper::LayerPage,
    layout::{
        transform::{Transform, TransformEdge, TransformValue},
        val::Val,
    },
    measures::Rectangle,
    theme::ThemeVal,
    widgets::{
        button::{ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{
            ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerEcho, ContainerWithCamera,
        },
        renderer::{
            rrect::RRect,
            text::{Text, TextAttr},
        },
    },
};

struct LayerChosen(u64);

pub struct LayerSelectionPanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for LayerSelectionPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("layer_selection");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let theme = world.single_fetch::<ThemeVal>();

        let attr = ContainerAttr::default();
        let (panel, common) = world.build(ContainerWithCamera {
            rect: self.rect,
            common: self.common,
            attr,
        });
        let restriction = world.build(ContainerDecoDropdown {
            height: None,
            outer: self.rect,
        });
        world.insert(ContainerDeco {
            container: panel,
            restriction,
        });
        let local = world.build(ContainerEcho { container: panel });

        let doc_label_rect = world.build(Transform::new(
            TransformValue::anchor((0.0, 1.0), Rectangle::new(28, -22 - 18, 28 + 32, -22)),
            local,
        ));
        world.insert(Text {
            text: tr("settings.layer_selection.doc_label").into(),
            rect: doc_label_rect,
            attr: TextAttr {
                metrics: Metrics {
                    font_size: 16.0,
                    line_height: 18.0,
                },
                attrs: Attrs::new(),
                ..Default::default()
            },
            order: 100,
            common,
            modulate: theme.distinct,
            outdated: false,
            canvas_outdated: false,
        });

        let div_rect = world.build(Transform::new(
            TransformValue {
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
            local,
        ));
        world.insert(RRect {
            common,
            rect: div_rect,
            order: 0,
            color: theme.gray2,
            radius: 0.0,
            width: 0.0,
        });

        let layer_label_rect = world.build(Transform::new(
            TransformValue::anchor((0.0, 1.0), Rectangle::new(28, -90, 28 + 32, -90 + 16)),
            local,
        ));
        world.insert(Text {
            text: tr("settings.layer_selection.layer_label").into(),
            rect: layer_label_rect,
            attr: TextAttr {
                metrics: Metrics {
                    font_size: 14.0,
                    line_height: 16.0,
                },
                attrs: Attrs::new(),
                ..Default::default()
            },
            order: 100,
            common,
            modulate: theme.distinct,
            outdated: false,
            canvas_outdated: false,
        });

        let layers_node = world.insert(());
        for i in 0..3i32 {
            let button_rect = world.build(Transform::new(
                TransformValue {
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
                local,
            ));
            let layer0_button = world.insert(ToggleButton {
                common: WidgetCommon {
                    camera: common.camera,
                    visible: self.common.visible,
                },
                rect: button_rect,
                theme: theme.button(),
                image: None,
                selected: i == 0,
                hovering: false,
            });

            let name_rect = world.build(Transform::new(
                TransformValue {
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
                button_rect,
            ));
            world.insert(Text {
                text: trp(
                    "settings.layer_selection.layer_name",
                    &[("index", &i.to_string()[..])],
                )
                .into(),
                rect: name_rect,
                attr: TextAttr {
                    metrics: Metrics {
                        font_size: 14.0,
                        line_height: 16.0,
                    },
                    attrs: Attrs::new(),
                    ..Default::default()
                },
                order: 100,
                common,
                modulate: theme.distinct,
                outdated: false,
                canvas_outdated: false,
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
        }
    }
}
