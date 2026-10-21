use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, HandleGeneric, World};

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
    theme::Theme,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible,
        button::{ButtonClick, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        panel::{Panel, PanelAttr, brush_panel::BrushPanel, color_picker::ColorPickerButton},
        renderer::{svg::svg_render, text::SetText},
        slider::{SetSliderValue, Slider, SliderLabel, SliderValue},
    },
};

pub struct SideDocker {
    pub common: WidgetCommon,
}

impl Element for SideDocker {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single_fetch::<Lnwindow>();
        let theme = world.single_fetch::<Theme>();
        let layer = world.single::<LayerPage>();

        world.named("side_docker_buttons");
        let side_panel = world.insert(Panel {
            common: self.common,
            rect: Rectangle::default(),
            attr: PanelAttr {
                shadow: true,
                round: true,
            },
        });

        let docker_button = |image_bytes| {
            world.insert(ToggleButton {
                common: self.common,
                rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
                theme: theme.button(),
                image: Some(svg_render(image_bytes, 8)),
                selected: false,
                hovering: false,
            })
        };

        let brush_menu = docker_button(include_bytes!("../../../res/interface/pen.svg"));
        let eraser = docker_button(include_bytes!("../../../res/interface/eraser.svg"));
        let undo = docker_button(include_bytes!("../../../res/interface/undo-2.svg"));
        let redo = docker_button(include_bytes!("../../../res/interface/redo-2.svg"));
        let touch = docker_button(include_bytes!("../../../res/interface/pointer.svg"));
        let pipette = docker_button(include_bytes!("../../../res/interface/pipette.svg"));
        let elastic_blank = world.insert(());

        let color_picker = world.insert(ColorPickerButton {
            common: self.common,
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
        });

        world.named("brush_panel");
        let brush_panel = world.insert(BrushPanel {
            common: self.common,
        });

        let slider = world.insert(Slider {
            common: self.common,
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(100)),
            axis: Axis::Up,
            value: 0.67,
            pressed: false,
        });

        let slider_label = world.insert(SliderLabel {
            text: String::new(),
            clockwise: true,
            source: slider,
            hover: false,
            visible: self.common.visible,
            camera: self.common.camera,
        });

        world.named("side_docker_observers");

        world.observer(brush_menu, move |&ButtonSelected(selected), world| {
            world.queue_trigger(brush_menu, SetButtonSelected(selected));
            world.queue_trigger(color_picker, SetButtonSelected(false));
        });

        world.observer(brush_menu, move |&SetButtonSelected(selected), world| {
            world.queue_trigger(brush_panel, SetWidgetVisible(selected));
        });

        world.observer(brush_menu, move |&SetWidgetRectangle(rect), world| {
            const PANEL_WIDTH: i32 = 364;
            const PANEL_HEIGHT: i32 = 480;
            let transform = TransformValue::anchor(
                (1.0, 1.0),
                Rectangle::new_extend(20, -PANEL_HEIGHT, PANEL_WIDTH as u32, PANEL_HEIGHT as u32),
            );
            let rect = transform.compute(rect);
            world.queue_trigger(brush_panel, SetWidgetRectangle(rect));
        });

        world.observer(color_picker, move |&ButtonSelected(selected), world| {
            world.queue_trigger(brush_menu, SetButtonSelected(false));
            world.queue_trigger(color_picker, SetButtonSelected(selected));
        });

        world.observer(eraser, move |&ButtonSelected(val), world| {
            let mut layer = world.fetch_mut(layer);
            layer.active_mut().set_toggle(BrushParamKey::Erase, val);
            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        world.observer(pipette, move |&ButtonSelected(val), world| {
            let mut input = world.single_fetch_mut::<LayerInput>();
            input.hold_pick = val;
            world.queue_trigger(pipette, SetButtonSelected(val));
        });

        world.observer(touch, move |&ButtonSelected(val), world| {
            let mut input = world.single_fetch_mut::<LayerInput>();
            input.touch_draw = val;
            world.queue_trigger(touch, SetButtonSelected(val));
        });

        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let is_eraser = {
                let layer = world.fetch(layer);
                layer.active().toggle(BrushParamKey::Erase).unwrap_or(false)
            };
            world.trigger(eraser, &SetButtonSelected(is_eraser));
        });

        world.observer(undo, move |&ButtonClick, world| {
            let mut layer = world.fetch_mut::<LayerPage>(layer);
            layer.undo();
        });

        world.observer(redo, move |&ButtonClick, world| {
            let mut layer = world.fetch_mut::<LayerPage>(layer);
            layer.redo();
        });

        world.observer(slider, move |&SliderValue(value), world| {
            let mut layer = world.fetch_mut(layer);
            let scale = (value.exp2() - 1.) * 127.5 + 0.5;
            if let Some(param) = layer.active_mut().scalar_mut(BrushParamKey::Size) {
                param.scale = scale;
            }

            world.queue_trigger(layer.handle(), BrushConfigurationChanged);
        });

        world.observer(layer, move |&BrushConfigurationChanged, world| {
            let value = {
                let layer = world.fetch(layer);
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

        let luni = world.insert(LuniFlex {
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

        world.dependency(side_panel, this);
        world.dependency(brush_menu, this);
        world.dependency(eraser, this);
        world.dependency(color_picker, this);
        world.dependency(undo, this);
        world.dependency(redo, this);
        world.dependency(elastic_blank, this);
        world.dependency(slider, this);
        world.dependency(touch, this);
        world.dependency(pipette, this);
        world.dependency(luni, this);
    }
}
