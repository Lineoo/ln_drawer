use ln_world::{Element, Handle, World};

use crate::{
    layer::{
        brush::param::BrushParamKey,
        input::LayerInput,
        wrapper::{BrushConfigurationChanged, LayerPage},
    },
    layout::{
        luni::{LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
        val::{Val, ValChanged},
    },
    measures::{Axis, Rectangle},
    theme::ThemeVal,
    widgets::{
        button::{ButtonClick, ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        panel::{Panel, PanelAttr, brush_panel::BrushPanel, color_picker::ColorPickerButton},
        renderer::{svg::svg_render, text::SetText},
        slider::{Slider, SliderLabel, SliderValue},
    },
};

pub struct SideDocker {
    pub common: WidgetCommon,
    pub rect: Handle<Val<Rectangle>>,
}

impl Element for SideDocker {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("side_docker");
        world.set_autodeps(Some(this));

        let theme = world.single_fetch::<ThemeVal>();
        let layer = world.single::<LayerPage>();

        let panel_rect = world.build(Transform::new(
            TransformValue {
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
            self.rect,
        ));

        world.insert(Panel {
            common: self.common,
            rect: panel_rect,
            attr: PanelAttr {
                shadow: true,
                round: true,
            },
        });

        let mut luni = LuniFlex::new(
            panel_rect,
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
        );

        let docker_button = |luni: &mut LuniFlex, image_bytes: &[u8]| {
            let rect = world.build(luni.child(LuniChild::default()));
            world.insert(ToggleButton {
                common: self.common,
                rect,
                theme: theme.button(),
                image: Some(ButtonImage::new(svg_render(image_bytes), theme.distinct, 8)),
                selected: world.insert(Val::new(false)),
                hovering: world.insert(Val::new(false)),
            })
        };

        let brush_menu_rect = world.build(luni.child(LuniChild::default()));
        let brush_menu = world.insert(ToggleButton {
            common: self.common,
            rect: brush_menu_rect,
            theme: theme.button(),
            image: Some(ButtonImage::new(
                svg_render(include_bytes!("../../../res/interface/pen.svg")),
                theme.distinct,
                8,
            )),
            selected: world.insert(Val::new(false)),
            hovering: world.insert(Val::new(false)),
        });

        let eraser = docker_button(
            &mut luni,
            include_bytes!("../../../res/interface/eraser.svg"),
        );

        let color_picker_rect = world.build(luni.child(LuniChild::default()));
        let color_picker = world.insert(ColorPickerButton {
            common: self.common,
            rect: color_picker_rect,
        });

        let undo = docker_button(
            &mut luni,
            include_bytes!("../../../res/interface/undo-2.svg"),
        );

        let redo = docker_button(
            &mut luni,
            include_bytes!("../../../res/interface/redo-2.svg"),
        );

        world.build(luni.child(LuniChild {
            basis: Some(0),
            grow: Some(1.0),
            ..Default::default()
        }));

        let slider_rect = world.build(luni.child(LuniChild {
            basis: Some(160),
            shrink: Some(1.0),
            margin: Some(LuniRect {
                left: 0,
                bottom: 5,
                right: 0,
                top: 5,
            }),
            ..Default::default()
        }));

        let touch = docker_button(
            &mut luni,
            include_bytes!("../../../res/interface/pointer.svg"),
        );

        let pipette = docker_button(
            &mut luni,
            include_bytes!("../../../res/interface/pipette.svg"),
        );

        let brush_panel_rect = world.build(Transform::anchor(
            (1.0, 1.0),
            Rectangle::new_extend(20, -480, 364, 480),
            brush_menu_rect,
        ));

        let brush_visible = world.insert(Val::new(false));
        world.insert(BrushPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: brush_visible,
            },
            rect: brush_panel_rect,
        });

        let value = world.insert(Val::new(0.67));
        let slider = world.insert(Slider {
            common: self.common,
            rect: slider_rect,
            axis: Axis::Up,
            value,
            pressed: false,
        });

        let slider_label = world.insert(SliderLabel {
            text: String::new(),
            clockwise: true,
            source: slider,
            hover: false,
            common: self.common,
        });

        world.observer(brush_menu, move |&ButtonSelected(selected), world| {
            world.queue_trigger(brush_menu, SetButtonSelected(selected));
            world.queue_trigger(color_picker, SetButtonSelected(false));
        });

        world.observer(brush_menu, move |&SetButtonSelected(selected), world| {
            let mut visible = world.fetch_mut(brush_visible);
            visible.val = selected;
            world.queue_trigger(visible.handle(), ValChanged);
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
            set_slider_value(world, slider, ((value - 0.5) / 127.5 + 1.).log2());
            world.queue_trigger(slider_label, SetText(format!("{value:.2} px")));
        });

        world.insert(luni);

        world.dependency(this, self.rect);
    }
}

fn set_slider_value(world: &World, slider: Handle<Slider>, next: f32) {
    let handle = world.fetch(slider).value;
    let mut value = world.fetch_mut(handle);
    if value.val != next {
        value.val = next;
        drop(value);
        world.queue_trigger(handle, ValChanged);
    }
}
