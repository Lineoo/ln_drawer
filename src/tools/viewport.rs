use crate::{
    measures::{Fract, PositionFract},
    render::viewport::Viewport,
    world::{Element, World},
};

#[derive(Default)]
pub struct ViewportUtils {
    /// same as the pointer
    cursor: [f64; 2],

    /// trying to follow your pointer
    anchor: [f64; 2],
    anchor_center: PositionFract,

    /// indicate whether it's locked
    locked: bool,
}

impl ViewportUtils {
    /// The behavior will depend on previous operations.
    pub fn cursor(&mut self, world: &World, cursor: [f64; 2]) {
        match self.locked {
            false => self.cursor_unlocked(world, cursor),
            true => self.cursor_locked(world, cursor),
        }
    }

    /// Viewport doesn't change. Anchor will just teleport to the cursor.
    pub fn cursor_unlocked(&mut self, world: &World, cursor: [f64; 2]) {
        let viewport = world.single_fetch::<Viewport>().unwrap();

        self.cursor = cursor;
        self.locked = false;

        self.anchor = cursor;
        self.anchor_center = viewport.center;
    }

    /// Viewport changes. Anchor will try best to follow the cursor by adjusting viewport.
    pub fn cursor_locked(&mut self, world: &World, cursor: [f64; 2]) {
        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

        self.cursor = cursor;
        self.locked = true;

        let delta = viewport.screen_to_world_relative([
            self.cursor[0] - self.anchor[0],
            self.cursor[1] - self.anchor[1],
        ]);

        viewport.center = self.anchor_center - delta;
    }

    /// Adjust zoom value, zooming in/out the anchor.
    pub fn zoom_delta(&mut self, world: &World, delta: Fract) {
        let mut viewport = world.single_fetch_mut::<Viewport>().unwrap();

        let world_cursor = viewport.screen_to_world_absolute(self.cursor);
        let follow = (viewport.center - world_cursor) * (-delta).exp2();

        viewport.zoom += delta;
        viewport.center = world_cursor + follow;

        self.anchor = self.cursor;
        self.anchor_center = viewport.center;
    }
}

impl Element for ViewportUtils {}
