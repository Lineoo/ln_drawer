use glam::{Vec2, Vec4};
use ln_world::Element;

use crate::{
    color::srgb::{hex_to_srgb, srgb_gamma_decode},
    measures::Axis,
    widgets::button::ToggleButtonTheme,
};

pub struct Theme {
    pub gray0: Vec4,
    pub gray1: Vec4,
    pub gray2: Vec4,
    pub gray3: Vec4,
    pub gray4: Vec4,
    pub theme: Vec4,
    pub distinct: Vec4,
    pub shadow_color: Vec4,
    pub shadow_offset: Vec2,
    pub shadow_blur: f32,
    pub roundness: f32,
    #[expect(unused)]
    pub axis: Axis,
    pub anim_factor: f32,
}

impl Theme {
    pub fn dark_mode() -> Self {
        Theme {
            gray0: srgb_gamma_decode(hex_to_srgb("#333")),
            gray1: srgb_gamma_decode(hex_to_srgb("#444")),
            gray2: srgb_gamma_decode(hex_to_srgb("#555")),
            gray3: srgb_gamma_decode(hex_to_srgb("#777")),
            gray4: srgb_gamma_decode(hex_to_srgb("#aaa")),
            theme: srgb_gamma_decode(hex_to_srgb("#cc7353")),
            distinct: srgb_gamma_decode(hex_to_srgb("#fff")),
            shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            roundness: 4.0,
            axis: Axis::Right,
            anim_factor: 30.0,
        }
    }

    pub fn light_mode() -> Self {
        Theme {
            gray0: srgb_gamma_decode(hex_to_srgb("#ffffff")),
            gray1: srgb_gamma_decode(hex_to_srgb("#f2f2f2")),
            gray2: srgb_gamma_decode(hex_to_srgb("#e5e5e5")),
            gray3: srgb_gamma_decode(hex_to_srgb("#dbdbdb")),
            gray4: srgb_gamma_decode(hex_to_srgb("#b8b8b8")),
            theme: srgb_gamma_decode(hex_to_srgb("#dcba96")),
            distinct: srgb_gamma_decode(hex_to_srgb("#000000")),
            shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            roundness: 4.0,
            axis: Axis::Right,
            anim_factor: 30.0,
        }
    }

    pub fn button(&self) -> ToggleButtonTheme {
        ToggleButtonTheme {
            idle_color: self.gray1,
            hover_color: self.gray2,
            press_color: self.gray3,
            selected_color: self.gray3,
        }
    }

    pub fn tab_button(&self) -> ToggleButtonTheme {
        ToggleButtonTheme {
            idle_color: self.gray2,
            hover_color: self.gray3,
            press_color: self.gray0,
            selected_color: self.gray0,
        }
    }
}

impl Element for Theme {}
