use std::sync::Arc;

use cosmic_text::{Attrs, Metrics};
use glam::{I64Vec2, IVec2, UVec2};
use ln_world::{ElemRef, Handle, HandleGeneric, ViewRef, World};

use crate::{
    i18n::tr,
    layer::{
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerWrapper},
    },
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::{RenderControl, RenderPhase, camera::CurrentCamera},
    theme::Theme,
    tools::collider::ToolColliderPortal,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{
            ButtonClick, ButtonDrag, ButtonDragStatus, ButtonImage, ButtonSelected,
            SetButtonSelected, ToggleButton, ToggleButtonTheme,
        },
        container::{Container, move_camera},
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
const ITEM_HEIGHT: i32 = 52;
const ITEM_GAP: i32 = 6;
const LIST_PADDING: i32 = 4;

pub fn brush_panel(world: &World, toggle_button: Handle<ToggleButton>) {
    let count = world.single_fetch::<LayerWrapper>().unwrap().brushes.len();

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
        inner_transform: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 0,
            },
            down: TransformEdge {
                anchor: 1.0,
                offset: -736,
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
    let wrapper = world.single::<LayerWrapper>().unwrap();
    for panel in [list_container, settings_container] {
        let control = world.insert(RenderControl::phase_with_draw(
            panel,
            move |world, rpass, extra| {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let camera = world.single_fetch::<CurrentCamera>().unwrap();
                let camera = world.fetch(camera.0).unwrap();
                let panel_rect = world.fetch(panel).unwrap().rect;
                let window_size = lnwindow.window.surface_size();
                let left_up = lnwindow.screen_to_cursor(
                    camera.world_to_screen_absolute(I64Vec2::q32_from_i32(panel_rect.left_up())),
                );
                let right_down = lnwindow.screen_to_cursor(
                    camera.world_to_screen_absolute(I64Vec2::q32_from_i32(panel_rect.right_down())),
                );
                rpass.set_scissor_rect(
                    (left_up.x as u32).max(0),
                    (left_up.y as u32).max(0),
                    (right_down.x as u32).min(window_size.width) - (left_up.x as u32),
                    (right_down.y as u32).min(window_size.height) - (left_up.y as u32),
                );
                world.enter(panel, || {
                    let phase = &mut *world.single_fetch_mut::<RenderPhase>().unwrap();
                    phase.reorder();
                    phase.draw(world, rpass, extra);
                });
                rpass.set_scissor_rect(0, 0, window_size.width, window_size.height);
            },
        ));
        RenderControl::reorder(Some(isize::MAX), world, control);
        world.enter(lnwindow, || {
            world.insert(ToolColliderPortal(panel.untyped()));
        });
        world.enter(panel, || {
            world.insert(ViewRef(lnwindow.untyped()));
            world.insert(ElemRef(input.untyped()));
            world.insert(ElemRef(panel.untyped()));
            world.insert(ElemRef(toggle_button.untyped()));
            world.insert(ElemRef(wrapper.untyped()));
            world.insert(RenderPhase::default());
        });
    }

    world.enter_queue(list_container, move |world| {
        brush_list(world, list_container)
    });
    world.enter_queue(settings_container, move |world| {
        super::settings::panel_settings(world, settings_container)
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

fn brush_list(world: &World, container: Handle<Container>) {
    let theme = world.single_fetch::<Theme>().unwrap();
    let wrapper = world.single::<LayerWrapper>().unwrap();
    let (count, active, labels) = {
        let layer = world.single_fetch::<LayerWrapper>().unwrap();
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
                font_size: 15.0,
                line_height: 18.0,
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
                    anchor: 0.5,
                    offset: -9,
                },
                right: TransformEdge {
                    anchor: 1.0,
                    offset: -14,
                },
                up: TransformEdge {
                    anchor: 0.5,
                    offset: 9,
                },
            },
            source: button.untyped(),
            target: label.untyped(),
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

        let mut last = None;
        world.observer(button, move |drag: &ButtonDrag, world| match drag.status {
            ButtonDragStatus::Start => {
                last = Some(drag.here.pointer.screen);
            }
            ButtonDragStatus::Dragging => {
                let screen = drag.here.pointer.screen;
                if let Some(prev) = last {
                    // Convert the screen-space delta with the current camera: a world-space
                    // delta would include the camera movement we are about to apply, feeding
                    // back into the next event and making the drag jitter.
                    let delta = world.enter(container, || {
                        let camera = world.single_fetch::<CurrentCamera>().unwrap();
                        let camera = world.fetch(camera.0).unwrap();
                        camera.screen_to_world_relative(screen - prev)
                    });
                    move_camera(world, container, delta.q32_as_f64());
                }
                last = Some(screen);
            }
            ButtonDragStatus::End => {
                last = None;
            }
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
