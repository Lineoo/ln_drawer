use std::time::Instant;

use palette::{Mix, Srgba};

use crate::{
    render::{RedrawPrepare, RenderControl},
    world::{Descriptor, Element, Handle, World},
};

pub struct Animation<T> {
    src: T,
    dst: T,

    factor: f32,
    target: f32,
    rate: f32,
    last_update: Instant,

    control: Handle<RenderControl>,
    control_active: bool,
}

pub struct AnimationDescriptor<T> {
    pub init: T,
    pub target: T,
    pub rate: f32,
}

pub struct AnimationValue<T>(pub T);

pub trait AnimationType: Copy + PartialEq + 'static {
    fn lerp(self, rhs: Self, factor: f32) -> Self;
    fn distance(self, rhs: Self) -> f32;
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
            src: self.init,
            dst: self.target,
            factor: 0.0,
            target: T::distance(self.init, self.target),
            rate: self.rate,
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
            let stepped = this.step();
            let changed = this.factor != stepped;
            this.factor = stepped;
            this.last_update = Instant::now();

            if changed {
                world.trigger(this.handle(), &AnimationValue(this.value()));
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
        if self.factor != self.target {
            self.control_active = true;
            self.last_update = Instant::now();
            let mut control = world.fetch_mut(self.control).unwrap();
            control.refreshing = true;
        }
    }
}

impl<T: AnimationType> Animation<T> {
    pub fn target(&mut self, value: T) {
        self.src = self.value();
        self.dst = value;
        self.factor = 0.0;
        self.target = T::distance(self.src, value);
    }

    pub fn value(&self) -> T {
        T::lerp(self.src, self.dst, self.factor / self.target)
    }

    fn step(&mut self) -> f32 {
        let delta = Instant::now() - self.last_update;
        (self.factor + self.rate * delta.as_secs_f32()).clamp(0.0, self.target)
    }
}

impl AnimationType for f32 {
    fn lerp(self, rhs: Self, factor: f32) -> Self {
        self * (1.0 - factor) + rhs * factor
    }

    fn distance(self, rhs: Self) -> f32 {
        (self - rhs).abs().max(1e-6)
    }
}

impl AnimationType for Srgba {
    fn lerp(self, rhs: Self, factor: f32) -> Self {
        self.mix(rhs, factor)
    }

    fn distance(self, _rhs: Self) -> f32 {
        1.0
    }
}
