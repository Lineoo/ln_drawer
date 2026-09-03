use glam::Vec2;
use ln_world::Element;
use palette::Srgba;

use crate::measures::Axis;

pub struct Theme {
    pub blank_color: Srgba,
    pub primary_color: Srgba,
    pub secondary_color: Srgba,
    pub highlight_color: Srgba,
    pub significant_color: Srgba,
    pub theme_color: Srgba,
    pub symbolic_color: Srgba,
    pub shadow_color: Srgba,
    pub shadow_offset: Vec2,
    pub shadow_blur: f32,
    pub roundness: f32,
    #[expect(unused)]
    pub axis: Axis,
    pub anim_factor: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            blank_color: Srgba::new(1.000, 1.000, 1.000, 1.0),
            primary_color: Srgba::new(0.949, 0.949, 0.949, 1.0),
            secondary_color: Srgba::new(0.898, 0.898, 0.898, 1.0),
            highlight_color: Srgba::new(0.859, 0.859, 0.859, 1.0),
            significant_color: Srgba::new(0.722, 0.722, 0.722, 1.0),
            theme_color: Srgba::new(0.863, 0.729, 0.588, 1.0),
            symbolic_color: Srgba::new(0.0, 0.0, 0.0, 1.0),
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.25),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            roundness: 4.0,
            axis: Axis::Right,
            anim_factor: 30.0,
        }
    }
}

impl Element for Theme {}
