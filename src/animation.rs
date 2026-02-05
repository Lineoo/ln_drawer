use std::time::Instant;

use palette::{Mix, Srgba};

use crate::{
    render::{RedrawPrepare, RenderControl},
    world::{Descriptor, Element, Handle, World},
};

pub struct Animation<T> {
    pub src: T,
    pub dst: T,
    pub factor: f32,

    last_update: Instant,

    control: Handle<RenderControl>,
    control_active: bool,
}

pub struct AnimationDescriptor<T> {
    pub src: T,
    pub dst: T,
    pub factor: f32,
}

pub struct AnimationValue<T>(pub T);

impl<T: AnimationType> Descriptor for AnimationDescriptor<T> {
    type Target = Handle<Animation<T>>;

    fn when_build(self, world: &World) -> Self::Target {
        let control = world.insert(RenderControl {
            visible: true,
            order: 0,
            refreshing: self.src != self.dst,
        });

        world.insert(Animation {
            src: self.src,
            dst: self.dst,
            factor: self.factor,
            last_update: Instant::now(),
            control,
            control_active: self.src != self.dst,
        })
    }
}

impl<T: AnimationType> Element for Animation<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(self.control, move |RedrawPrepare, world, control| {
            let mut this = world.fetch_mut(this).unwrap();

            let now = Instant::now();
            let delta = (now - this.last_update).as_secs_f32();
            let factor = f32::exp(-this.factor * delta);
            let next = T::lerp(this.src, this.dst, 1.0 - factor);
            let changed = this.src != next;

            this.src = next;
            this.last_update = now;

            if changed {
                world.trigger(this.handle(), &AnimationValue(next));
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
        if self.src != self.dst {
            self.control_active = true;
            self.last_update = Instant::now();
            let mut control = world.fetch_mut(self.control).unwrap();
            control.refreshing = true;
        }
    }
}

trait AnimationType: Copy + PartialEq + 'static {
    /// The `factor` is usually close to 0, which stands for `rhs` rather than `self`.
    /// Note that a built-in snapping should be provided.
    fn lerp(self, rhs: Self, factor: f32) -> Self;
}

impl AnimationType for f32 {
    fn lerp(self, rhs: Self, factor: f32) -> Self {
        if (self - rhs).abs() < 1e-2 {
            return rhs;
        }

        self * (1.0 - factor) + rhs * factor
    }
}

impl AnimationType for Srgba {
    fn lerp(self, rhs: Self, factor: f32) -> Self {
        if (self.red - rhs.red).abs() < 1e-2
            && (self.green - rhs.green).abs() < 1e-2
            && (self.blue - rhs.blue).abs() < 1e-2
            && (self.alpha - rhs.alpha).abs() < 1e-2
        {
            return rhs;
        }

        self.mix(rhs, factor)
    }
}
