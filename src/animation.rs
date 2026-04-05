use std::time::{Duration, Instant};

use palette::Srgba;

use crate::{
    lnwin::Lnwindow,
    render::{RenderControl, RenderInformation},
    world::{Descriptor, Element, Handle, RefMut, World},
};

pub struct Animation<T: AnimationEasingType> {
    pub src: T,
    pub dst: T,
    pub factor: f32,

    last_update: Instant,
}

pub struct AnimationDescriptor<T: AnimationEasingType> {
    pub src: T,
    pub dst: T,
    pub factor: f32,
}

impl<T: AnimationEasingType> AnimationDescriptor<T> {
    pub fn new(init: T, factor: f32) -> Self {
        Self {
            src: init,
            dst: init,
            factor,
        }
    }
}

impl<T: AnimationEasingType> Descriptor for AnimationDescriptor<T> {
    type Target = Handle<Animation<T>>;

    fn when_build(self, world: &World) -> Self::Target {
        world.insert(Animation {
            src: self.src,
            dst: self.dst,
            factor: self.factor,
            last_update: Instant::now(),
        })
    }
}

pub struct SimpleAnimationDescriptor<T, W, F>
where
    T: AnimationEasingType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    pub animation: AnimationDescriptor<T>,
    pub widget: Handle<W>,
    pub action: F,
}

impl<T, W, F> Descriptor for SimpleAnimationDescriptor<T, W, F>
where
    T: AnimationEasingType,
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

pub struct OnceAnimationDescriptor<T, W, F>
where
    T: AnimationEasingType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    pub animation: AnimationDescriptor<T>,
    pub widget: Handle<W>,
    pub action: F,
}

impl<T, W, F> Descriptor for OnceAnimationDescriptor<T, W, F>
where
    T: AnimationEasingType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    type Target = Handle<Animation<T>>;

    fn when_build(mut self, world: &World) -> Self::Target {
        let dst = self.animation.dst;
        let anim = world.build(self.animation);
        world.dependency(anim, self.widget);
        world.observer(anim, move |&AnimationValue::<T>(value), world| {
            let widget = world.fetch_mut(self.widget).unwrap();
            (self.action)(widget, world, value);

            if value == dst {
                world.queue(move |world| {
                    world.remove(anim).unwrap();
                });
            }
        });

        anim
    }
}

impl<T: AnimationEasingType> Element for Animation<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();

                // calculate next value

                let now = Instant::now();

                let the = &mut *this;
                let changed = T::step(
                    &mut the.src,
                    &mut the.dst,
                    the.factor,
                    now - the.last_update,
                );

                this.last_update = now;

                // send event and change RenderControl

                if changed {
                    world.trigger(this.handle(), &AnimationValue(this.src));
                }

                Some(RenderInformation {
                    render_order: 0,
                    keep_redrawing: this.src != this.dst,
                })
            })),
            draw: None,
        });

        world.dependency(control, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        world.queue_trigger(this, AnimationValue(self.src));
        if self.src != self.dst {
            self.last_update = Instant::now();
            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            lnwindow.window.request_redraw();
        }
    }
}

pub struct AnimationValue<T: AnimationEasingType>(pub T);

pub trait AnimationType: PartialEq + Clone + Copy + 'static {
    fn step(&mut self, rhs: &mut Self, factor: f32, delta: Duration) -> bool;
}

pub trait AnimationEasingType: PartialEq + Clone + Copy + 'static {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32>;
}

impl<T: AnimationEasingType> AnimationType for T {
    fn step(&mut self, rhs: &mut Self, factor: f32, delta: Duration) -> bool {
        let delta = delta.as_secs_f32();
        let factor = f32::exp(-factor * delta);

        let mut changed = false;
        let iter = Iterator::zip(self.float_iter().into_iter(), rhs.float_iter().into_iter());

        for (src_ref, dst_ref) in iter {
            let (src, dst) = (*src_ref, *dst_ref);

            let next = match (src - dst).abs() < 1e-2 {
                true => dst, // snap
                false => src * factor + dst * (1.0 - factor),
            };

            changed |= src != next;
            *src_ref = next;
        }

        changed
    }
}

impl AnimationEasingType for f32 {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        [self]
    }
}

impl<const N: usize> AnimationEasingType for [f32; N] {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        self
    }
}

impl AnimationEasingType for Srgba {
    fn float_iter(&mut self) -> impl IntoIterator<Item = &mut f32> {
        [
            &mut self.color.red,
            &mut self.color.green,
            &mut self.color.blue,
            &mut self.alpha,
        ]
    }
}
