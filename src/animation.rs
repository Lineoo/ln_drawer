use std::time::Instant;

use crate::{
    render::{RedrawPrepare, RenderControl},
    world::{Commander, Descriptor, Element, Handle, World},
};

pub struct Animation<T> {
    current: T,
    target: T,
    factor: f32,
    last_update: Instant,

    control: Handle<RenderControl>,
    control_active: bool,
    need_redraw: bool,
}

pub struct AnimationDescriptor<T> {
    pub init: T,
    pub factor: f32,
}

pub struct AnimationValue<T>(pub T);

impl Descriptor for AnimationDescriptor<f32> {
    type Target = Handle<Animation<f32>>;

    fn when_build(self, world: &World) -> Self::Target {
        let control = world.insert(RenderControl {
            visible: true,
            order: 0,
            refreshing: false,
        });

        world.insert(Animation {
            current: self.init,
            target: self.init,
            factor: self.factor,
            last_update: Instant::now(),
            control,
            control_active: false,
            need_redraw: false,
        })
    }
}

impl Element for Animation<f32> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.control, move |RedrawPrepare, world, control| {
            let mut this = world.fetch_mut(this).unwrap();
            let (value, changed) = this.update();

            if this.control_active != changed {
                this.control_active = changed;

                let mut control = world.fetch_mut(control).unwrap();
                control.refreshing = changed;
            }

            if changed {
                world.trigger(this.handle(), &AnimationValue(value));
            }
        });

        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        if self.need_redraw {
            self.need_redraw = false;
            let mut control = world.fetch_mut(self.control).unwrap();
            control.refreshing = true;
        }
    }
}

impl Animation<f32> {
    fn update(&mut self) -> (f32, bool) {
        let delta = (Instant::now() - self.last_update).as_secs_f32();
        let dest = self.current + (self.target - self.current).signum() * self.factor * delta;
        let clamped = dest.clamp(self.current.min(self.target), self.current.max(self.target));
        let changed = self.current != clamped;

        self.current = clamped;
        self.last_update = Instant::now();

        (clamped, changed)
    }
}

impl<T> Animation<T> {
    pub fn target(&mut self, value: T) {
        self.target = value;
        self.last_update = Instant::now();
        self.need_redraw = true;
    }
}
