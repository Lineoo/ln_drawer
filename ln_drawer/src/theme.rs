use ln_world::Element;
use palette::Srgba;

pub struct ColorScheme {
    pub color: Srgba,
    pub active_color: Srgba,
    pub press_color: Srgba,
    pub roundness: f32,
    pub press_roundness: f32,
    pub anim_factor: f32,
    pub anim_factor_menu: f32,
    pub pad: i32,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            color: Srgba::new(0.863, 0.863, 0.863, 1.0),
            active_color: Srgba::new(0.808, 0.808, 0.808, 1.0),
            press_color: Srgba::new(0.737, 0.737, 0.737, 1.0),
            roundness: 5.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
        }
    }
}

impl Element for ColorScheme {}
