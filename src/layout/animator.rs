use std::time::Duration;

use crate::{
    animation::{Animation, AnimationDescriptor, AnimationValue},
    layout::Layout,
    measures::Rectangle,
    world::{Descriptor, Element, Handle, World},
};

pub struct Animator {
    animation: Handle<Animation<f32>>,
    src: Rectangle,
    dst: Rectangle,
    target: Handle,
}

pub struct AnimatorDescriptor {
    pub src: Rectangle,
    pub dst: Rectangle,
    pub time: Duration,
    pub target: Handle,
}

impl Descriptor for AnimatorDescriptor {
    type Target = Handle<Animator>;

    fn when_build(self, world: &World) -> Self::Target {
        world.insert(Animator {
            animation: world.build(AnimationDescriptor {
                init: 0.0,
                target: 1.0,
                factor: 1.0 / self.time.as_secs_f32(),
            }),
            src: self.src,
            dst: self.dst,
            target: self.target,
        })
    }
}

impl Element for Animator {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.dependency(self.animation, this);

        let target = self.target;
        world.observer(
            self.animation,
            move |&AnimationValue::<f32>(val), world, _| {
                let this = world.fetch(this).unwrap();
                world.trigger(
                    target,
                    &Layout::Rectangle(Rectangle::lerp(this.src, this.dst, val)),
                );
            },
        );
    }
}
