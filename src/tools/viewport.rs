use crate::{
    measures::{Fract, PositionFract},
    render::viewport::Viewport,
    world::{Element, World},
};

#[derive(Default)]
pub struct ViewportUtils {
    cursor: [f64; 2],

    // viewport: PositionFract      = viewport.center
    // cursor_in_viewport: [f64; 2] = cursor
    anchor: PositionFract,
    cursor_in_anchor: [f64; 2],

    locked: bool,
}

impl ViewportUtils {
    /// Adjust zoom value, zooming in/out the anchor.
    pub fn zoom_delta(&mut self, world: &World, delta: Fract) {
        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
        let zoom_center = viewport.screen_to_world_absolute(self.cursor);

        let anchor_origin = self.anchor;
        self.anchor = zoom_center;
        self.cursor_in_anchor = [0.0, 0.0];

        viewport.zoom += delta;
        drop(viewport);

        self.update_locked(world);

        self.anchor = anchor_origin;
        self.update_unlocked(world);
    }

    pub fn cursor(&mut self, world: &World, cursor: [f64; 2]) {
        self.cursor = cursor;
        self.update(world);
    }

    pub fn anchor(&mut self, world: &World, anchor: PositionFract) {
        self.anchor = anchor;
        self.update(world);
    }

    pub fn anchor_on_screen(&mut self, world: &World, anchor_on_screen: [f64; 2]) {
        let viewport = world.single_fetch::<Viewport>().unwrap();
        let anchor = viewport.screen_to_world_absolute(anchor_on_screen);
        drop(viewport);
        self.anchor(world, anchor);
    }

    /// Set **locked** to change viewport.
    pub fn locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    /// The behavior will depend on previous operations.
    fn update(&mut self, world: &World) {
        if self.locked {
            self.update_locked(world);
        } else {
            self.update_unlocked(world);
        }
    }

    /// resolve `viewport.center`
    fn update_locked(&mut self, world: &World) {
        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();
        let delta = viewport.screen_to_world_relative([
            self.cursor[0] - self.cursor_in_anchor[0],
            self.cursor[1] - self.cursor_in_anchor[1],
        ]);

        viewport.center = self.anchor - delta;
    }

    /// resolve `cursor_in_anchor`
    fn update_unlocked(&mut self, world: &World) {
        let viewport = world.single_fetch::<Viewport>().unwrap();
        let delta = viewport.world_to_screen_relative(self.anchor - viewport.center);

        self.cursor_in_anchor = [self.cursor[0] - delta[0], self.cursor[1] - delta[1]];
    }
}

impl Element for ViewportUtils {}
