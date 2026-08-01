use glam::{I64Vec2, IVec2, UVec2};
use ln_world::{Descriptor, World};
use palette::Srgba;

use crate::{
    layer::{modifier::Modifier, wrapper::LayerWrapper}, layout::{
        luni::{LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
    }, lnwin::Lnwindow, measures::{Axis, Rectangle}, render::camera::{Camera, MainCamera}, save::SaveDatabase, theme::Theme, widgets::{
        WidgetClick, WidgetRectangle, button::{Button, ButtonChecked, ButtonImage}, panel::{color_picker::ColorPicker, debug_panel::DebugPanel}, slider::{SetSliderValue, Slider, SliderOutput},
    },
};

pub struct SideDocker;
impl Descriptor for SideDocker {
    type Target = ();
    fn when_build(self, world: &World) -> Self::Target {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let side_panel = world.insert(Button {
            order: 0,
            color: theme.primary_color,
            active_color: theme.primary_color,
            press_color: theme.primary_color,
            roundness: theme.roundness,
            ..Default::default()
        });

        let docker_button = |image| {
            world.insert(Button {
                order: 10,
                color: theme.primary_color,
                active_color: theme.secondary_color,
                press_color: theme.highlight_color,
                shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
                roundness: theme.roundness,
                image: Some(ButtonImage {
                    transform: TransformValue::anchor(
                        (0.5, 0.5),
                        Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
                    ),
                    bytes: image,
                }),
                ..Default::default()
            })
        };

        let pen = docker_button(include_bytes!("../../../res/interface/pen.png"));
        let brush = docker_button(include_bytes!("../../../res/interface/brush.png"));
        let eraser = docker_button(include_bytes!("../../../res/interface/eraser.png"));
        let undo = docker_button(include_bytes!("../../../res/interface/undo-2.png"));
        let redo = docker_button(include_bytes!("../../../res/interface/redo-2.png"));

        let color_picker = world.insert(Button {
            order: 10,
            color: theme.primary_color,
            active_color: theme.secondary_color,
            press_color: theme.highlight_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            roundness: theme.roundness,
            image: None,
            ..Default::default()
        });

        world.build(ColorPicker(color_picker));

        let elastic_blank = world.insert(());

        let slider = world.build(Slider {
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
            axis: Axis::Up,
            value: 0.67,
        });

        world.observer(pen, move |&WidgetClick, world| {
            world.trigger(pen, &ButtonChecked(true));
            world.trigger(brush, &ButtonChecked(false));
            world.trigger(eraser, &ButtonChecked(false));
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

        world.observer(brush, move |&WidgetClick, world| {
            world.trigger(pen, &ButtonChecked(false));
            world.trigger(brush, &ButtonChecked(true));
            world.trigger(eraser, &ButtonChecked(false));
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

        world.observer(eraser, move |&WidgetClick, world| {
            world.trigger(pen, &ButtonChecked(false));
            world.trigger(brush, &ButtonChecked(false));
            world.trigger(eraser, &ButtonChecked(true));
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

        world.observer(undo, move |&WidgetClick, world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.undo();
        });

        world.observer(redo, move |&WidgetClick, world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.redo();
        });

        world.observer(slider, move |&SliderOutput(value), world| {
            let mut stroke = world.single_fetch_mut::<LayerWrapper>().unwrap();
            stroke.brush.modifier = Modifier {
                max_size: stroke.brush.modifier.min_size + (value.exp2() - 1.0) * 40.0,
                ..stroke.brush.modifier
            };
            world.trigger(slider, &SetSliderValue(value));
        });

        let compass = docker_button(include_bytes!("../../../res/interface/compass.png"));

        world.observer(compass, move |&WidgetClick, world| {
            let main_camera = world.single_fetch::<MainCamera>().unwrap();
            let mut camera = world
                .enter_single_fetch_mut::<Camera>(main_camera.0)
                .unwrap();
            camera.center = I64Vec2::ZERO;
        });

        let debug = docker_button(include_bytes!("../../../res/interface/bug.png"));
        world.build(DebugPanel(debug));

        let compact = docker_button(include_bytes!("../../../res/interface/folder-down.png"));
        world.observer(compact, move |&WidgetClick, world| {
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
            source: lnwindow.untyped(),
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

        world.queue_trigger(side_panel, WidgetRectangle(Rectangle::new(0, 0, 500, 100)));
    }
}
