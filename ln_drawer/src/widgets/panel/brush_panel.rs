use std::sync::Arc;

use cosmic_text::{Attrs, Metrics};
use glam::{IVec2, UVec2};
use ln_world::{ElemRef, Handle, HandleGeneric, ViewRef, World};

use crate::{
    i18n::tr,
    layer::{
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::Rectangle,
    theme::Theme,
    tools::pointer::{PointerHit, PointerScroll},
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        brush_preview::{BrushPreview, BrushPreviewGenerator},
        button::{
            ButtonClick, ButtonDrag, ButtonDragStatus, ButtonImage, ButtonSelected,
            SetButtonSelected, ToggleButton, ToggleButtonTheme,
        },
        container::Container,
        echo::Echo,
        renderer::{
            rrect::{RRect, SetRRectColor},
            svg::svg_render,
            text::Text,
        },
        tabs::{SetTabsActive, Tabs},
    },
};

const PANEL_WIDTH: i32 = 364;
const PANEL_HEIGHT: i32 = 480;
const ITEM_HEIGHT: i32 = 80;
const ITEM_GAP: i32 = 6;
const LIST_PADDING: i32 = 8;

pub fn brush_panel(world: &World, toggle_button: Handle<ToggleButton>) {
    let count = world.single_fetch::<LayerPage>().unwrap().brushes.len();

    let content_height =
        count as i32 * ITEM_HEIGHT + (count as i32 - 1).max(0) * ITEM_GAP + LIST_PADDING * 2;

    let list_container = world.insert(Container {
        rect: Rectangle::default(),
        inner: Rectangle::default(),
        inner_transform: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 0,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -content_height,
            },
            right: TransformEdge {
                anchor: 1.0,
                offset: 0,
            },
            up: TransformEdge {
                anchor: 1.0,
                offset: 0,
            },
        },
        visible: false,
    });

    let settings_container = world.insert(Container {
        rect: Rectangle::default(),
        inner: Rectangle::default(),
        inner_transform: TransformValue::copy(),
        visible: false,
    });

    let tabs = world.insert(Tabs {
        active: 0,
        rect: Rectangle::default(),
        visible: false,
        tabs: vec![
            (
                tab_icon(include_bytes!("../../../res/interface/brush.svg")),
                list_container.untyped(),
            ),
            (
                tab_icon(include_bytes!("../../../res/interface/settings.svg")),
                settings_container.untyped(),
            ),
        ],
    });

    let lnwindow = world.single::<Lnwindow>().unwrap();
    let input = world.single::<LayerInput>().unwrap();
    let wrapper = world.single::<LayerPage>().unwrap();
    let wrapper_instance = world.fetch(wrapper).unwrap();
    let generator = world.insert(BrushPreviewGenerator::new(
        wrapper_instance.draw.layer.clone(),
    ));
    for panel in [list_container, settings_container] {
        world.enter(panel, || {
            world.insert(ViewRef(lnwindow.untyped()));
            world.insert(ElemRef(input.untyped()));
            world.insert(ElemRef(panel.untyped()));
            world.insert(ElemRef(toggle_button.untyped()));
            world.insert(ElemRef(wrapper.untyped()));
            world.insert(ElemRef(generator.untyped()));
        });
    }

    world.enter_queue(list_container, move |world| {
        brush_list(world, list_container, generator)
    });
    world.enter_queue(settings_container, move |world| {
        super::settings::new_panel_settings(world, settings_container, generator)
    });

    world.observer(toggle_button, move |&SetWidgetRectangle(rect), world| {
        let transform = TransformValue::anchor(
            (1.0, 1.0),
            Rectangle::new_extend(20, -PANEL_HEIGHT, PANEL_WIDTH as u32, PANEL_HEIGHT as u32),
        );
        let rect = transform.compute(rect);
        world.queue_trigger(tabs, SetWidgetRectangle(rect));
    });

    world.observer(toggle_button, move |&ButtonSelected(selected), world| {
        world.queue_trigger(toggle_button, SetButtonSelected(selected));
    });

    world.observer(toggle_button, move |&SetButtonSelected(selected), world| {
        world.queue_trigger(tabs, SetWidgetVisible(selected));
    });

    // initialize layout
    world.queue(move |world| {
        let this = world.fetch(tabs).unwrap();
        world.queue_trigger(tabs, SetWidgetRectangle(this.rect));
        world.queue_trigger(tabs, SetWidgetVisible(this.visible));
        world.queue_trigger(tabs, SetTabsActive(this.active));
    });
}

fn brush_list(
    world: &World,
    container: Handle<Container>,
    generator: Handle<BrushPreviewGenerator>,
) {
    let theme = world.single_fetch::<Theme>().unwrap();
    let wrapper = world.single::<LayerPage>().unwrap();
    let (count, active, labels) = {
        let layer = world.single_fetch::<LayerPage>().unwrap();
        let labels = layer
            .brushes
            .iter()
            .map(|preset| preset.label)
            .collect::<Vec<_>>();
        (layer.brushes.len(), layer.active_brush, labels)
    };

    let accent = theme.theme_color;
    let background = theme.secondary_color;
    let symbolic = theme.symbolic_color;

    let mut children = Vec::new();
    for i in 0..count {
        let button = world.insert(ToggleButton {
            rect: Rectangle::default(),
            theme: ToggleButtonTheme {
                idle_color: background,
                hover_color: theme.highlight_color,
                press_color: theme.highlight_color,
                selected_color: background,
            },
            image: None,
            selected: i == active,
            visible: true,
            hovering: false,
        });

        Echo::new(world, button).widget_rectangle().widget_visible();

        let outline = world.insert(RRect {
            rect: Rectangle::default(),
            order: 9,
            color: if i == active { accent } else { background },
            radius: theme.roundness,
            width: 0.0,
            enabled: true,
        });

        world.insert(Transform {
            value: TransformValue::shrink(-2, -2),
            source: button.untyped(),
            target: outline.untyped(),
        });

        let label = world.insert(Text {
            text: tr(labels[i]).into(),
            metrics: Metrics {
                font_size: 12.0,
                line_height: 14.0,
            },
            attrs: Attrs::new(),
            color: symbolic,
            ..Default::default()
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 14,
                },
                down: TransformEdge {
                    anchor: 1.0,
                    offset: -4 - 14,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: -14,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -4,
                },
            },
            source: button.untyped(),
            target: label.untyped(),
        });

        let wrapper_instance = world.fetch(wrapper).unwrap();
        let preview = world.insert(BrushPreview {
            rect: Rectangle::default(),
            generator,
            outdated: true,
            brush: wrapper_instance.brushes.get(i).map(|x| x.brush.dup()),
            visible: false,
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 0,
                },
                down: TransformEdge {
                    anchor: 0.0,
                    offset: 0,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: 0,
                },
                up: TransformEdge {
                    anchor: 1.0,
                    offset: -20,
                },
            },
            source: button.untyped(),
            target: preview.untyped(),
        });

        world.observer(button, move |&ButtonClick, world| {
            let mut wrapper = world.fetch_mut(wrapper).unwrap();
            if wrapper.active_brush == i {
                return;
            }
            wrapper.select_brush(i);
            world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
        });

        world.observer(wrapper, move |&BrushConfigurationChanged, world| {
            let active = world.fetch(wrapper).unwrap().active_brush;
            let selected = i == active;
            world.queue_trigger(button, SetButtonSelected(selected));
            world.queue_trigger(
                outline,
                SetRRectColor(match selected {
                    true => accent,
                    false => background,
                }),
            );
        });

        world.observer(button, move |drag: &ButtonDrag, world| match drag.status {
            ButtonDragStatus::Start => {
                world.trigger(
                    container,
                    &PointerHit {
                        position: drag.here.position,
                        pointer: drag.here.pointer,
                        status: drag.from.status,
                        data: drag.here.data,
                    },
                );
            }
            ButtonDragStatus::Dragging | ButtonDragStatus::End => {
                world.trigger(
                    container,
                    &PointerHit {
                        position: drag.here.position,
                        pointer: drag.here.pointer,
                        status: drag.here.status,
                        data: drag.here.data,
                    },
                );
            }
        });

        world.observer(button, move |scroll: &PointerScroll, world| {
            world.trigger(container, scroll);
        });

        children.push((button.untyped(), LuniChild::default()));
    }

    world.insert(LuniFlex {
        parent: (
            container.untyped(),
            LuniParent {
                axis: LuniAxis::Column,
                distribution: LuniDistribution::FlexStart,
                padding: LuniRect {
                    left: 8,
                    bottom: LIST_PADDING,
                    right: 8,
                    top: LIST_PADDING,
                },
                gap: ITEM_GAP,
                template: LuniChildTemplate {
                    align: LuniAlign::Stretch,
                    basis: ITEM_HEIGHT,
                    ..Default::default()
                },
            },
        ),
        children,
    });
}

fn tab_icon(bytes: &[u8]) -> ButtonImage {
    ButtonImage {
        transform: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(12)),
        ),
        bytes: Arc::new(image::DynamicImage::from(svg_render(bytes, 1.0))),
    }
}
