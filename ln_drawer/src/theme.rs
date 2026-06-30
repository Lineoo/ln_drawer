use ln_world::Element;
use palette::Srgba;

pub struct Theme {
    pub primary_color: Srgba,
    pub secondary_color: Srgba,
    pub highlight_color: Srgba,
    pub significant_color: Srgba,
    pub theme_color: Srgba,
    pub symbolic_color: Srgba,
    pub shadow_color: Srgba,

    pub roundness: f32,
    pub press_roundness: f32,
    pub anim_factor: f32,
    pub pad: i32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            primary_color: Srgba::new(0.949, 0.949, 0.949, 1.0),
            secondary_color: Srgba::new(0.898, 0.898, 0.898, 1.0),
            highlight_color: Srgba::new(0.859, 0.859, 0.859, 1.0),
            significant_color: Srgba::new(0.722, 0.722, 0.722, 1.0),
            theme_color: Srgba::new(0.863, 0.729, 0.588, 1.0),
            symbolic_color: Srgba::new(0.0, 0.0, 0.0, 1.0),
            shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.5),
            roundness: 5.0,
            press_roundness: 15.0,
            anim_factor: 30.0,
            pad: 5,
        }
    }
}

impl Element for Theme {}
