use glam::{Vec2, Vec4};
use ln_world::Element;

use crate::{
    color::srgb::{hex_to_srgb, srgb_gamma_decode},
    measures::Axis,
};

const DARK_MODE: bool = true;

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
        match DARK_MODE {
            true => Theme {
                blank_color: srgb_gamma_decode(hex_to_srgb("#333")),
                primary_color: srgb_gamma_decode(hex_to_srgb("#444")),
                secondary_color: srgb_gamma_decode(hex_to_srgb("#555")),
                highlight_color: srgb_gamma_decode(hex_to_srgb("#777")),
                significant_color: srgb_gamma_decode(hex_to_srgb("#aaa")),
                theme_color: srgb_gamma_decode(hex_to_srgb("#cc7353")),
                symbolic_color: srgb_gamma_decode(hex_to_srgb("#fff")),
                shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
                shadow_offset: Vec2::new(0.0, -4.0),
                shadow_blur: 4.0,
                roundness: 4.0,
                axis: Axis::Right,
                anim_factor: 30.0,
            },
            false => Theme {
                blank_color: srgb_gamma_decode(hex_to_srgb("#ffffff")),
                primary_color: srgb_gamma_decode(hex_to_srgb("#f2f2f2")),
                secondary_color: srgb_gamma_decode(hex_to_srgb("#e5e5e5")),
                highlight_color: srgb_gamma_decode(hex_to_srgb("#dbdbdb")),
                significant_color: srgb_gamma_decode(hex_to_srgb("#b8b8b8")),
                theme_color: srgb_gamma_decode(hex_to_srgb("#dcba96")),
                symbolic_color: srgb_gamma_decode(hex_to_srgb("#000000")),
                shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
                shadow_offset: Vec2::new(0.0, -4.0),
                shadow_blur: 4.0,
                roundness: 4.0,
                axis: Axis::Right,
                anim_factor: 30.0,
            },
        }
    }
}

impl Element for Theme {}
