use std::time::Instant;

use palette::Srgba;

use crate::{
    render::{RedrawPrepare, RenderControl},
    world::{Descriptor, Element, Handle, RefMut, World},
};

pub struct Animation<T: AnimationType> {
    pub src: T,
    pub dst: T,
    pub factor: f32,

    last_update: Instant,

    control: Handle<RenderControl>,
    control_active: bool,
}

pub struct AnimationDescriptor<T: AnimationType> {
    pub src: T,
    pub dst: T,
    pub factor: f32,
}

impl<T: AnimationType> AnimationDescriptor<T> {
    pub fn new(init: T, factor: f32) -> Self {
        Self {
            src: init,
            dst: init,
            factor,
        }
    }
}

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

pub struct SimpleAnimationDescriptor<T, W, F>
where
    T: AnimationType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    pub animation: AnimationDescriptor<T>,
    pub widget: Handle<W>,
    pub action: F,
}

impl<T, W, F> Descriptor for SimpleAnimationDescriptor<T, W, F>
where
    T: AnimationType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    type Target = Handle<Animation<T>>;

    fn when_build(mut self, world: &World) -> Self::Target {
        let anim = world.build(self.animation);
        world.dependency(anim, self.widget);
        world.observer(anim, move |&AnimationValue::<T>(value), world| {
            let widget = world.fetch_mut(self.widget).unwrap();
            (self.action)(widget, world, value);
        });

        anim
    }
}

impl<T: AnimationType> Element for Animation<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = self.control;
        world.observer(control, move |RedrawPrepare, world| {
            let mut this = world.fetch_mut(this).unwrap();

            // calculate next value

            let now = Instant::now();
            let delta = (now - this.last_update).as_secs_f32();
            let factor = f32::exp(-this.factor * delta);
            this.last_update = now;

            let mut changed = false;
            let the = &mut *this;
            let iter = Iterator::zip(
                the.src.float_iter().into_iter(),
                the.dst.float_iter().into_iter(),
            );

            for (src_ref, dst_ref) in iter {
                let (src, dst) = (*src_ref, *dst_ref);

                let next = match (src - dst).abs() < 1e-2 {
                    true => dst, // snap
                    false => src * factor + dst * (1.0 - factor),
                };

                changed |= src != next;
                *src_ref = next;
            }

            // send event and change RenderControl

            if changed {
                world.trigger(this.handle(), &AnimationValue(this.src));
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

pub struct AnimationValue<T: AnimationType>(pub T);

pub trait AnimationType: PartialEq + Clone + Copy + 'static {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32>;
}

impl AnimationType for f32 {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        [self]
    }
}

impl<const N: usize> AnimationType for [f32; N] {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        self
    }
}

impl AnimationType for Srgba {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        [
            &mut self.color.red,
            &mut self.color.green,
            &mut self.color.blue,
            &mut self.alpha,
        ]
    }
}
