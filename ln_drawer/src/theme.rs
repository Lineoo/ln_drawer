use ln_world::Element;
use palette::Srgba;

/// `Luni` stands for `ln_ui`. It's this basic widgets' render implementation of ln_drawer.
pub struct Luni {
    pub color: Srgba,
    pub active_color: Srgba,
    pub press_color: Srgba,
    pub roundness: f32,
    pub press_roundness: f32,
    pub anim_factor: f32,
    pub anim_factor_menu: f32,
    pub pad: i32,
}

impl Default for Luni {
    fn default() -> Self {
        Self {
            color: Srgba::new(0.1, 0.1, 0.1, 0.9),
            active_color: Srgba::new(0.3, 0.3, 0.3, 1.0),
            press_color: Srgba::new(0.2, 0.2, 0.2, 1.0),
            roundness: 5.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            anim_factor_menu: 50.0,
            pad: 5,
        }
    }
}

impl Element for Luni {}
