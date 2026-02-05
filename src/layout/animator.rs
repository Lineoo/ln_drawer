use crate::{
    animation::{Animation, AnimationDescriptor, AnimationValue},
    layout::Layout,
    measures::Rectangle,
    world::{Descriptor, Element, Handle, World},
};

pub struct Animator {
    src: Rectangle,
    dst: Rectangle,
    target: Handle,
    animation: Handle<Animation<f32>>,
}

pub struct AnimatorDescriptor {
    pub src: Rectangle,
    pub dst: Rectangle,
    pub rate: f32,
    pub target: Handle,
}

impl Descriptor for AnimatorDescriptor {
    type Target = Handle<Animator>;

    fn when_build(self, world: &World) -> Self::Target {
        let animation = world.build(AnimationDescriptor {
            src: 0.0,
            dst: 1.0,
            factor: self.rate,
        });

        world.insert(Animator {
            src: self.src,
            dst: self.dst,
            target: self.target,
            animation,
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
