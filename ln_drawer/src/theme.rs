use glam::{Vec2, Vec4};
use ln_world::{Element, Handle, World};

use crate::{
    color::{
        oklab::oklab_to_linear_srgb,
        srgb::{hex_to_srgb, srgb_gamma_decode},
    },
    layout::val::{Val, ValChanged},
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
    /// Brush preview band colors, in L.
    pub c0: Vec4,
    pub c1: Vec4,
    pub shadow_color: Vec4,
    pub shadow_offset: Vec2,
    pub shadow_blur: f32,
    pub roundness: f32,
    pub axis: Axis,
    pub anim_factor: f32,
}

pub struct ThemeVal {
    pub gray0: Handle<Val<Vec4>>,
    pub gray1: Handle<Val<Vec4>>,
    pub gray2: Handle<Val<Vec4>>,
    pub gray3: Handle<Val<Vec4>>,
    pub gray4: Handle<Val<Vec4>>,
    pub theme: Handle<Val<Vec4>>,
    pub distinct: Handle<Val<Vec4>>,
    pub c0: Handle<Val<Vec4>>,
    pub c1: Handle<Val<Vec4>>,
    pub shadow_color: Handle<Val<Vec4>>,
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
            c0: oklab_to_linear_srgb(Vec4::new(0.6, 0.08, 0.0, 1.0)),
            c1: oklab_to_linear_srgb(Vec4::new(0.5, 0.0, 0.0, 1.0)),
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
            c0: oklab_to_linear_srgb(Vec4::new(0.90, 0.1, 0.0, 1.0)),
            c1: oklab_to_linear_srgb(Vec4::new(0.98, 0.0, 0.0, 1.0)),
            shadow_color: srgb_gamma_decode(Vec4::new(0.0, 0.0, 0.0, 0.25)),
            shadow_offset: Vec2::new(0.0, -4.0),
            shadow_blur: 4.0,
            roundness: 4.0,
            axis: Axis::Right,
            anim_factor: 30.0,
        }
    }

    pub fn bind(&self, world: &World) -> ThemeVal {
        ThemeVal {
            gray0: world.insert(Val::new(self.gray0)),
            gray1: world.insert(Val::new(self.gray1)),
            gray2: world.insert(Val::new(self.gray2)),
            gray3: world.insert(Val::new(self.gray3)),
            gray4: world.insert(Val::new(self.gray4)),
            theme: world.insert(Val::new(self.theme)),
            distinct: world.insert(Val::new(self.distinct)),
            c0: world.insert(Val::new(self.c0)),
            c1: world.insert(Val::new(self.c1)),
            shadow_color: world.insert(Val::new(self.shadow_color)),
        }
    }

    pub fn sync(&self, world: &World) {
        let bind = world.single_fetch::<ThemeVal>();

        world.fetch_mut(bind.gray0).val = self.gray0;
        world.fetch_mut(bind.gray1).val = self.gray1;
        world.fetch_mut(bind.gray2).val = self.gray2;
        world.fetch_mut(bind.gray3).val = self.gray3;
        world.fetch_mut(bind.gray4).val = self.gray4;
        world.fetch_mut(bind.theme).val = self.theme;
        world.fetch_mut(bind.distinct).val = self.distinct;
        world.fetch_mut(bind.c0).val = self.c0;
        world.fetch_mut(bind.c1).val = self.c1;
        world.fetch_mut(bind.shadow_color).val = self.shadow_color;

        world.queue_trigger(bind.gray0, ValChanged);
        world.queue_trigger(bind.gray1, ValChanged);
        world.queue_trigger(bind.gray2, ValChanged);
        world.queue_trigger(bind.gray3, ValChanged);
        world.queue_trigger(bind.gray4, ValChanged);
        world.queue_trigger(bind.theme, ValChanged);
        world.queue_trigger(bind.distinct, ValChanged);
        world.queue_trigger(bind.c0, ValChanged);
        world.queue_trigger(bind.c1, ValChanged);
        world.queue_trigger(bind.shadow_color, ValChanged);
    }
}

impl ThemeVal {
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
impl Element for ThemeVal {}
