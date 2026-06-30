use glam::Vec2;
use ln_world::{Element, Handle, World};
use palette::Srgba;

use crate::{
    measures::{Position, Rectangle, Size},
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
};

pub struct VSlider {
    pub x: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

impl VSlider {
    pub fn create_renderer(slider: Handle<VSlider>, world: &mut World) {
        let instance = world.fetch(slider).unwrap();
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.build(RoundedRectDescriptor {
            rect: Rectangle::new(
                instance.x - 10,
                instance.y_min,
                instance.x + 10,
                instance.y_max,
            ),
            color: theme.secondary_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: true,
            order: 0,
        });

        let value = ((instance.value - instance.min) / (instance.max - instance.min)
            * (instance.y_max - instance.y_min) as f32)
            .round() as i32
            + instance.y_min;

        let front = world.build(RoundedRectDescriptor {
            rect: Rectangle::new(instance.x - 10, instance.y_min, instance.x + 10, value),
            color: theme.theme_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: true,
            order: 1,
        });

        let knob = world.build(RoundedRectDescriptor {
            rect: Rectangle::new_half(Position::new(instance.x, value), Size::new(12, 25)),
            color: theme.primary_color,
            shadow_color: theme.shadow_color,
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 10.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 20,
            visible: true,
            order: 2,
        });

        let knob_split = world.build(RoundedRectDescriptor {
            rect: Rectangle::new_half(Position::new(instance.x, value), Size::new(8, 2)),
            color: theme.secondary_color,
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
            shadow_offset: Vec2::ZERO,
            shadow_blur: 0.0,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 0,
            visible: true,
            order: 3,
        });
    }
}

impl Element for VSlider {}
