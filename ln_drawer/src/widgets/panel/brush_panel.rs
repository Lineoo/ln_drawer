use cosmic_text::{Attrs, Metrics};
use glam::{DVec2, I64Vec2};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    i18n::tr,
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::{
        echo::Echo,
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::Rectangle,
    render::camera::Camera,
    theme::Theme,
    tools::pointer::PointerHit,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        brush_preview::{BrushPreview, BrushPreviewGenerator},
        button::{
            ButtonClick, ButtonDrag, ButtonDragStatus, SetButtonSelected, ToggleButton,
            ToggleButtonTheme,
        },
        common::WidgetCommon,
        container::{Container, ContainerDeco, ContainerDecoDropdown, ContainerWithCamera},
        renderer::{
            rrect::{RRect, SetRRectColor},
            svg::svg_render,
            text::Text,
        },
        tabs::{SetTabsActive, Tabs},
    },
};

const ITEM_HEIGHT: i32 = 80;
const ITEM_GAP: i32 = 6;
const LIST_PADDING: i32 = 8;

pub struct BrushPanel {
    pub common: WidgetCommon,
}

impl Element for BrushPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let page = world.single_fetch::<LayerPage>();
        let count = page.brushes.len();

        let generator = world.insert(BrushPreviewGenerator {
            lp: page.lp.clone(),
        });

        let content_height =
            count as i32 * ITEM_HEIGHT + (count as i32 - 1).max(0) * ITEM_GAP + LIST_PADDING * 2;

        let (list_container, cam_list) = world.build(ContainerWithCamera {
            common: self.common,
            rect: Rectangle::default(),
            scroll: I64Vec2::ZERO,
            anchor: DVec2::new(0.0, 1.0),
        });
        let list_container_deco = world.insert(ContainerDeco {
            container: list_container,
            restriction: Rectangle::default(),
        });
        world.insert(ContainerDecoDropdown {
            height: Some(content_height),
            container: list_container,
            deco: list_container_deco,
        });

        let (list_settings, cam_settings) = world.build(ContainerWithCamera {
            rect: Rectangle::default(),
            common: self.common,
            scroll: I64Vec2::ZERO,
            anchor: DVec2::new(0.0, 1.0),
        });
        let list_settings_deco = world.insert(ContainerDeco {
            container: list_settings,
            restriction: Rectangle::default(),
        });
        world.insert(ContainerDecoDropdown {
            height: None,
            container: list_settings,
            deco: list_settings_deco,
        });

        let tabs = world.insert(Tabs {
            active: 0,
            rect: Rectangle::default(),
            visible: self.common.visible,
            tabs: vec![
                (
                    svg_render(include_bytes!("../../../res/interface/brush.svg"), 12),
                    list_container.untyped(),
                ),
                (
                    svg_render(include_bytes!("../../../res/interface/settings.svg"), 12),
                    list_settings.untyped(),
                ),
            ],
            camera: self.common.camera,
        });

        Transfer::new(world, this, tabs)
            .set_widget_rectangle()
            .set_widget_visible();

        world.queue(move |world| brush_list(world, list_container, generator, cam_list));
        world.queue(move |world| {
            super::settings::new_panel_settings(world, list_settings, generator, cam_settings)
        });

        world.dependency(list_container, this);
        world.dependency(list_settings, this);
        world.dependency(tabs, this);
        world.dependency(generator, this);

        // initialize layout
        world.queue(move |world| {
            let this = world.fetch(tabs);
            world.queue_trigger(tabs, SetWidgetRectangle(this.rect));
            world.queue_trigger(tabs, SetWidgetVisible(this.visible));
            world.queue_trigger(tabs, SetTabsActive(this.active));
        });
    }
}

fn brush_list(
    world: &World,
    container: Handle<Container>,
    generator: Handle<BrushPreviewGenerator>,
    camera: Handle<Camera>,
) {
    let theme = world.single_fetch::<Theme>();
    let wrapper = world.single::<LayerPage>();
    let (count, active, labels) = {
        let layer = world.single_fetch::<LayerPage>();
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
            common: WidgetCommon {
                camera,
                visible: true,
            },
            rect: Rectangle::default(),
            theme: ToggleButtonTheme {
                idle_color: background,
                hover_color: theme.highlight_color,
                press_color: theme.highlight_color,
                selected_color: background,
            },
            image: None,
            selected: i == active,
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
            camera,
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
            camera: Some(camera),
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

        let wrapper_instance = world.fetch(wrapper);
        let preview = world.insert(BrushPreview {
            rect: Rectangle::default(),
            generator,
            outdated: true,
            brush: wrapper_instance.brushes.get(i).map(|x| x.brush.dup()),
            visible: false,
            camera,
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

        world.dependency(button, container);
        world.dependency(outline, container);
        world.dependency(label, container);
        world.dependency(preview, container);

        world.observer(button, move |&ButtonClick, world| {
            let mut wrapper = world.fetch_mut(wrapper);
            if wrapper.active_brush == i {
                return;
            }
            wrapper.select_brush(i);
            world.queue_trigger(wrapper.handle(), BrushConfigurationChanged);
        });

        world.observer(wrapper, move |&BrushConfigurationChanged, world| {
            let active = world.fetch(wrapper).active_brush;
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

        // TODO temporary: `Container` couples render order with collider order, so the preview
        // captures pointer events. Forward them to the button until that is decoupled.
        Transfer::new(world, preview, button)
            .pointer_hit()
            .pointer_hover()
            .pointer_scroll();

        Transfer::new(world, button, container).pointer_scroll();

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
