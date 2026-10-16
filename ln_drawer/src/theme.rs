use glam::{Vec2, Vec4};
use ln_world::Element;

use crate::{color::srgb::srgb_gamma_decode, measures::Axis};

pub struct Theme {
    pub blank_color: Vec4,
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    pub highlight_color: Vec4,
    pub significant_color: Vec4,
    pub theme_color: Vec4,
    pub symbolic_color: Vec4,
    pub shadow_color: Vec4,
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
            blank_color: srgb_gamma_decode(Vec4::new(1.000, 1.000, 1.000, 1.0)),
            primary_color: srgb_gamma_decode(Vec4::new(0.949, 0.949, 0.949, 1.0)),
            secondary_color: srgb_gamma_decode(Vec4::new(0.898, 0.898, 0.898, 1.0)),
            highlight_color: srgb_gamma_decode(Vec4::new(0.859, 0.859, 0.859, 1.0)),
            significant_color: srgb_gamma_decode(Vec4::new(0.722, 0.722, 0.722, 1.0)),
            theme_color: srgb_gamma_decode(Vec4::new(0.863, 0.729, 0.588, 1.0)),
            symbolic_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 1.0)),
            shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            roundness: 4.0,
            axis: Axis::Right,
            anim_factor: 30.0,
        }
    }
}

impl Element for Theme {}
