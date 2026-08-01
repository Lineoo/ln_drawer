use std::sync::Arc;

use glam::{I64Vec2, IVec2, UVec2};
use ln_world::{Descriptor, World};

use crate::{
    layer::{modifier::Modifier, wrapper::LayerWrapper},
    layout::{
        luni::{LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::{Axis, Rectangle},
    render::camera::{Camera, MainCamera},
    save::SaveDatabase,
    theme::Theme,
    widgets::{
        ButtonClick, SetWidgetRectangle,
        button::{ButtonImage, SetButtonSelected, ToggleButton, ToggleButtonTheme},
        panel::{Panel, color_picker::ColorPicker, debug_panel::DebugPanel},
        renderer::svg::svg_render,
        slider::{SetSliderValue, Slider, SliderValue},
    },
};

pub struct SideDocker;
impl Descriptor for SideDocker {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let side_panel = world.build(Panel {
            rect: Rectangle::default(),
            visible: true,
            shadow: true,
        });

        let docker_button = |image_bytes| {
            world.build(ToggleButton {
                rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
                theme: ToggleButtonTheme {
                    idle_color: theme.primary_color,
                    hover_color: theme.secondary_color,
                    press_color: theme.highlight_color,
                    selected_color: theme.highlight_color,
                },
                image: Some(ButtonImage {
                    transform: TransformValue::anchor(
                        (0.5, 0.5),
                        Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
                    ),
                    bytes: Arc::new(image::DynamicImage::from(svg_render(image_bytes, 1.0))),
                }),
                selected: false,
                visible: true,
                hovering: false,
            })
        };

        let pen = docker_button(include_bytes!("../../../res/interface/pen.svg"));
        let brush = docker_button(include_bytes!("../../../res/interface/brush.svg"));
        let eraser = docker_button(include_bytes!("../../../res/interface/eraser.svg"));
        let undo = docker_button(include_bytes!("../../../res/interface/undo-2.svg"));
        let redo = docker_button(include_bytes!("../../../res/interface/redo-2.svg"));

        let color_picker = world.build(ToggleButton {
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
            theme: ToggleButtonTheme {
                idle_color: theme.primary_color,
                hover_color: theme.secondary_color,
                press_color: theme.highlight_color,
                selected_color: theme.highlight_color,
            },
            image: None,
            selected: false,
            visible: true,
            hovering: false,
        });

        world.build(ColorPicker(color_picker));

        let elastic_blank = world.insert(());

        let slider = world.build(Slider {
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
            axis: Axis::Up,
            value: 0.67,
        });

        world.observer(pen, move |&ButtonClick, world| {
            world.trigger(pen, &SetButtonSelected(true));
            world.trigger(brush, &SetButtonSelected(false));
            world.trigger(eraser, &SetButtonSelected(false));
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.brush.modifier = Modifier {
                min_size: 0.0,
                max_size: 6.0,
                size_force_exp: 1.0,
                min_flow: 0.7,
                max_flow: 1.0,
                flow_force_exp: 2.0,
                softness: 0.2,
                ..stroke.brush.modifier
            };
            stroke.brush.erase = false;

            let slider = world.fetch(slider).unwrap();
            world.queue_trigger(
                slider.handle(),
                SetSliderValue((6.0f32 / 40.0 + 1.0).log2()),
            );
        });

        world.observer(brush, move |&ButtonClick, world| {
            world.trigger(pen, &SetButtonSelected(false));
            world.trigger(brush, &SetButtonSelected(true));
            world.trigger(eraser, &SetButtonSelected(false));
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.brush.modifier = Modifier {
                min_size: 1.0,
                max_size: 25.0,
                size_force_exp: 1.0,
                min_flow: 0.1,
                max_flow: 1.0,
                flow_force_exp: 1.0,
                softness: 0.5,
                ..stroke.brush.modifier
            };
            stroke.brush.erase = false;

            let slider = world.fetch(slider).unwrap();
            world.queue_trigger(
                slider.handle(),
                SetSliderValue((24.0f32 / 40.0 + 1.0).log2()),
            );
        });

        world.observer(eraser, move |&ButtonClick, world| {
            world.trigger(pen, &SetButtonSelected(false));
            world.trigger(brush, &SetButtonSelected(false));
            world.trigger(eraser, &SetButtonSelected(true));
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.brush.modifier = Modifier {
                min_size: 10.0,
                max_size: 50.0,
                size_force_exp: 1.0,
                min_flow: 0.1,
                max_flow: 1.0,
                flow_force_exp: 1.0,
                softness: 0.5,
                ..stroke.brush.modifier
            };
            stroke.brush.erase = true;

            let slider = world.fetch(slider).unwrap();
            world.queue_trigger(
                slider.handle(),
                SetSliderValue((40.0f32 / 40.0 + 1.0).log2()),
            );
        });

        world.observer(undo, move |&ButtonClick, world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.undo();
        });

        world.observer(redo, move |&ButtonClick, world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.redo();
        });

        world.observer(slider, move |&SliderValue(value), world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.brush.modifier = Modifier {
                max_size: stroke.brush.modifier.min_size + (value.exp2() - 1.0) * 40.0,
                ..stroke.brush.modifier
            };
            world.trigger(slider, &SetSliderValue(value));
        });

        let compass = docker_button(include_bytes!("../../../res/interface/compass.svg"));

        world.observer(compass, move |&ButtonClick, world| {
            let main_camera = world.single_fetch::<MainCamera>().unwrap();
            let mut camera = world
                .enter_single_fetch_mut::<Camera>(main_camera.0)
                .unwrap();
            camera.center = I64Vec2::ZERO;
        });

        let debug = docker_button(include_bytes!("../../../res/interface/bug.svg"));
        world.build(DebugPanel(debug));

        let compact = docker_button(include_bytes!("../../../res/interface/database-zap.svg"));
        world.observer(compact, move |&ButtonClick, world| {
            let db = world.single_fetch::<SaveDatabase>().unwrap();
            log::debug!("on next startup database will be compacted");
            SaveDatabase::write_compact(&db.0).unwrap();
        });

        world.insert(Transform {
            value: TransformValue {
                left: TransformEdge {
                    anchor: 0.0,
                    offset: 24,
                },
                down: TransformEdge {
                    anchor: 0.5,
                    offset: -280,
                },
                right: TransformEdge {
                    anchor: 0.0,
                    offset: 24 + 44,
                },
                up: TransformEdge {
                    anchor: 0.5,
                    offset: 280,
                },
            },
            source: lnwindow.handle().untyped(),
            target: side_panel.untyped(),
        });

        world.insert(LuniFlex {
            parent: (
                side_panel.untyped(),
                LuniParent {
                    axis: LuniAxis::Column,
                    template: LuniChildTemplate {
                        basis: 36,
                        cross: 36,
                        align: LuniAlign::Center,
                        ..Default::default()
                    },
                    padding: LuniRect {
                        left: 0,
                        bottom: 4,
                        right: 0,
                        top: 4,
                    },
                    gap: 4,
                    ..Default::default()
                },
            ),
            children: vec![
                (pen.untyped(), LuniChild::default()),
                (brush.untyped(), LuniChild::default()),
                (eraser.untyped(), LuniChild::default()),
                (color_picker.untyped(), LuniChild::default()),
                (undo.untyped(), LuniChild::default()),
                (redo.untyped(), LuniChild::default()),
                (
                    elastic_blank.untyped(),
                    LuniChild {
                        basis: Some(0),
                        grow: Some(1.0),
                        ..Default::default()
                    },
                ),
                (
                    slider.untyped(),
                    LuniChild {
                        basis: Some(160),
                        shrink: Some(1.0),
                        margin: Some(LuniRect {
                            left: 0,
                            bottom: 5,
                            right: 0,
                            top: 5,
                        }),
                        ..Default::default()
                    },
                ),
                (compass.untyped(), LuniChild::default()),
                (debug.untyped(), LuniChild::default()),
                (compact.untyped(), LuniChild::default()),
            ],
        });

        world.queue_trigger(
            side_panel,
            SetWidgetRectangle(Rectangle::new(0, 0, 500, 100)),
        );
    }
}
