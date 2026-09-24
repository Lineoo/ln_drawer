use std::sync::Arc;

use glam::{IVec2, UVec2};
use ln_world::{Handle, HandleGeneric, World};

use crate::{
    layer::{
        brush::param::BrushParamKey,
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        luni::{LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
    },
    lnwin::Lnwindow,
    measures::{Axis, Rectangle},
    render::camera::Camera,
    theme::Theme,
    widgets::{
        button::{
            ButtonClick, ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton,
            ToggleButtonTheme,
        },
        panel::{Panel, brush_panel::brush_panel, color_picker::color_picker_panel},
        renderer::{svg::svg_render, text::SetText},
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
    },
};

pub fn side_docker(world: &World, camera: Handle<Camera>) {
    let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
    let theme = world.single_fetch::<Theme>().unwrap();
    let layer = world.single::<LayerPage>().unwrap();

    let side_panel = world.insert(Panel {
        rect: Rectangle::default(),
        visible: true,
        shadow: true,
        camera,
    });

    let docker_button = |image_bytes| {
        world.insert(ToggleButton {
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
            camera,
        })
    };

    let brush_menu = docker_button(include_bytes!("../../../res/interface/pen.svg"));
    let eraser = docker_button(include_bytes!("../../../res/interface/eraser.svg"));
    let undo = docker_button(include_bytes!("../../../res/interface/undo-2.svg"));
    let redo = docker_button(include_bytes!("../../../res/interface/redo-2.svg"));
    let touch = docker_button(include_bytes!("../../../res/interface/pointer.svg"));
    let pipette = docker_button(include_bytes!("../../../res/interface/pipette.svg"));

    let color_picker = world.insert(ToggleButton {
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
        camera,
    });

    color_picker_panel(world, color_picker, camera);
    brush_panel(world, brush_menu, camera);

    // Only one popup may be open at a time, otherwise the overlapping panels cross.
    world.observer(brush_menu, move |&ButtonSelected(selected), world| {
        if selected {
            world.queue_trigger(color_picker, SetButtonSelected(false));
        }
    });

    world.observer(color_picker, move |&ButtonSelected(selected), world| {
        if selected {
            world.queue_trigger(brush_menu, SetButtonSelected(false));
        }
    });

    let elastic_blank = world.insert(());

    let slider = world.insert(Slider {
        rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
        axis: Axis::Up,
        value: 0.67,
        pressed: false,
        camera,
    });

    let slider_label = world.insert(SliderLabel {
        text: String::new(),
        clockwise: true,
        source: slider,
        hover: false,
        visible: true,
        camera,
    });

    world.observer(eraser, move |&ButtonSelected(val), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        layer.active_mut().set_toggle(BrushParamKey::Erase, val);
        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    world.observer(pipette, move |&ButtonSelected(val), world| {
        let mut input = world.single_fetch_mut::<LayerInput>().unwrap();
        input.hold_pick = val;
        world.queue_trigger(pipette, SetButtonSelected(val));
    });

    world.observer(touch, move |&ButtonSelected(val), world| {
        let mut input = world.single_fetch_mut::<LayerInput>().unwrap();
        input.touch_draw = val;
        world.queue_trigger(touch, SetButtonSelected(val));
    });

    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let is_eraser = {
            let layer = world.fetch(layer).unwrap();
            layer.active().toggle(BrushParamKey::Erase).unwrap_or(false)
        };
        world.trigger(eraser, &SetButtonSelected(is_eraser));
    });

    world.observer(undo, move |&ButtonClick, world| {
        let mut layer = world.fetch_mut::<LayerPage>(layer).unwrap();
        layer.undo();
    });

    world.observer(redo, move |&ButtonClick, world| {
        let mut layer = world.fetch_mut::<LayerPage>(layer).unwrap();
        layer.redo();
    });

    world.observer(slider, move |&SliderValue(value), world| {
        let mut layer = world.fetch_mut(layer).unwrap();
        let scale = (value.exp2() - 1.) * 127.5 + 0.5;
        if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::Size) {
            param.scale = scale;
        }

        world.queue_trigger(layer.handle(), BrushConfigurationChanged);
    });

    world.observer(layer, move |&BrushConfigurationChanged, world| {
        let value = {
            let layer = world.fetch(layer).unwrap();
            layer
                .active()
                .scalar(BrushParamKey::Size)
                .map(|param| param.scale)
                .unwrap_or(0.0)
        };
        world.trigger(slider, &SetSliderValue(((value - 0.5) / 127.5 + 1.).log2()));
        world.queue_trigger(slider_label, SetText(format!("{value:.2} px")));
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
            (brush_menu.untyped(), LuniChild::default()),
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
            (touch.untyped(), LuniChild::default()),
            (pipette.untyped(), LuniChild::default()),
        ],
    });

    world.queue_trigger(layer, BrushConfigurationChanged);
}
