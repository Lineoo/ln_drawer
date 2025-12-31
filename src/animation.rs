use std::time::{Duration, Instant};

use crate::{
    render::{RedrawPrepare, RenderControl},
    world::{Descriptor, Element, Handle, World},
};

pub struct Animation<T> {
    current: T,
    target: T,
    factor: f32,
    last_update: Instant,

    control: Handle<RenderControl>,
    control_active: bool,
}

pub struct AnimationDescriptor<T> {
    pub init: T,
    pub target: T,
    pub factor: f32,
}

pub struct AnimationValue<T>(pub T);

pub trait AnimationType: Copy + PartialEq + 'static {
    fn step(anim: &mut Animation<Self>, delta: Duration) -> Self;
}

impl<T: AnimationType> Descriptor for AnimationDescriptor<T> {
    type Target = Handle<Animation<T>>;

    fn when_build(self, world: &World) -> Self::Target {
        let control = world.insert(RenderControl {
            visible: true,
            order: 0,
            refreshing: self.init != self.target,
        });

        world.insert(Animation {
            current: self.init,
            target: self.target,
            factor: self.factor,
            last_update: Instant::now(),
            control,
            control_active: self.init != self.target,
        })
    }
}

impl<T: AnimationType> Element for Animation<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.control, move |RedrawPrepare, world, control| {
            let mut this = world.fetch_mut(this).unwrap();
            let (value, changed) = this.update();

            if changed {
                world.trigger(this.handle(), &AnimationValue(value));
            }

            if this.control_active != changed {
                this.control_active = changed;

                let mut control = world.fetch_mut(control).unwrap();
                control.refreshing = changed;
            }
        });

        world.dependency(self.control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        if self.current != self.target {
            self.control_active = true;
            self.last_update = Instant::now();
            let mut control = world.fetch_mut(self.control).unwrap();
            control.refreshing = true;
        }
    }
}

impl<T: AnimationType> Animation<T> {
    fn update(&mut self) -> (T, bool) {
        let clamped = T::step(self, Instant::now() - self.last_update);
        let changed = self.current != clamped;

        self.current = clamped;
        self.last_update = Instant::now();

        (clamped, changed)
    }

    pub fn reset(&mut self, value: T) {
        self.current = value;
    }

    pub fn target(&mut self, value: T) {
        self.target = value;
    }
}

impl AnimationType for f32 {
    fn step(anim: &mut Animation<Self>, delta: Duration) -> Self {
        let dest = anim.current
            + (anim.target - anim.current).signum() * anim.factor * delta.as_secs_f32();
        dest.clamp(anim.current.min(anim.target), anim.current.max(anim.target))
    }
}
