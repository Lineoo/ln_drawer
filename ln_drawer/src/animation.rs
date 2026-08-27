use std::time::{Duration, Instant};

use ln_world::{Descriptor, Element, Handle, RefMut, World};
use palette::Srgba;

use crate::{
    measures::Rectangle,
    render::{RenderControl, RenderInformation},
};

pub struct Animation<T: AnimationType> {
    pub src: T::Storage,
    pub dst: T::Storage,
    pub factor: f32,

    data_pushed: T,
    last_update: Instant,
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
        world.insert(Animation {
            src: self.src.into_storage(),
            dst: self.dst.into_storage(),
            factor: self.factor,
            data_pushed: self.src,
            last_update: Instant::now(),
        })
    }
}

#[expect(unused)]
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

#[expect(unused)]
pub struct OnceAnimationDescriptor<T, W, F>
where
    T: AnimationType,
    W: Element,
    F: FnMut(RefMut<W>, &World, T) + 'static,
{
    pub animation: AnimationDescriptor<T>,
    pub widget: Handle<W>,
    pub action: F,
}

impl<T, W, F> Descriptor for OnceAnimationDescriptor<T, W, F>
where
    T: AnimationType,
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

pub struct SetAnimationDst<T: AnimationType>(pub T);

pub struct DirectAnimation<T, W>
where
    T: AnimationType,
    W: Element,
{
    pub init: T,
    pub factor: f32,
    pub widget: Handle<W>,
    pub access: for<'a> fn(&'a mut W) -> &'a mut T,
}

impl<T, W> Descriptor for DirectAnimation<T, W>
where
    T: AnimationType,
    W: Element,
{
    type Target = Handle<Animation<T>>;

    fn when_build(self, world: &World) -> Self::Target {
        let anim = world.build(AnimationDescriptor::new(self.init, self.factor));
        world.dependency(anim, self.widget);
        world.observer(anim, move |&AnimationValue::<T>(value), world| {
            let mut widget = world.fetch_mut(self.widget).unwrap();
            let access = (self.access)(&mut widget);
            *access = value;
        });

        anim
    }
}

impl<T: AnimationType> Element for Animation<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let mut this = world.fetch_mut(this).unwrap();

                // calculate next value

                let now = Instant::now();

                let the = &mut *this;
                let changed = step::<T>(
                    &mut the.src,
                    &mut the.dst,
                    the.factor,
                    now - the.last_update,
                );

                this.last_update = now;

                // send event and change RenderControl

                let new_data = T::from_storage(this.src);
                if changed || this.data_pushed != new_data {
                    world.trigger(this.handle(), &AnimationValue(new_data));
                    this.data_pushed = new_data;
                }

                Some(RenderInformation {
                    keep_redrawing: this.src != this.dst,
                })
            })),
            draw: None,
        });

        world.observer(this, move |&SetAnimationDst(dst), world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.dst = T::into_storage(dst);
        });

        world.dependency(control, this);
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        if self.src != self.dst || self.data_pushed != T::from_storage(self.src) {
            self.last_update = Instant::now();
            RenderControl::redraw(world);
        }
    }
}

pub struct AnimationValue<T: AnimationType>(pub T);

fn step<T: AnimationType>(
    val: &mut T::Storage,
    rhs: &mut T::Storage,
    factor: f32,
    delta: Duration,
) -> bool {
    let delta = delta.as_secs_f32();
    let factor = f32::exp(-factor * delta);

    let mut changed = false;

    let iter = Iterator::zip(
        val.as_float_slice().into_iter(),
        rhs.as_float_slice().into_iter(),
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

    changed
}

pub trait AnimationType: PartialEq + Clone + Copy + 'static {
    type Storage: FloatArray;
    fn into_storage(self) -> Self::Storage;
    fn from_storage(storage: Self::Storage) -> Self;
}

impl AnimationType for f32 {
    type Storage = [f32; 1];

    fn into_storage(self) -> Self::Storage {
        [self]
    }

    fn from_storage(storage: Self::Storage) -> Self {
        storage[0]
    }
}

impl<const N: usize> AnimationType for [f32; N] {
    type Storage = [f32; N];

    fn into_storage(self) -> Self::Storage {
        self
    }

    fn from_storage(storage: Self::Storage) -> Self {
        storage
    }
}

impl AnimationType for Srgba {
    type Storage = [f32; 4];

    fn into_storage(self) -> Self::Storage {
        self.into()
    }

    fn from_storage(storage: Self::Storage) -> Self {
        storage.into()
    }
}

impl AnimationType for Rectangle {
    type Storage = [f32; 4];

    fn into_storage(self) -> Self::Storage {
        [
            self.left() as f32,
            self.down() as f32,
            self.right() as f32,
            self.up() as f32,
        ]
    }

    fn from_storage(storage: Self::Storage) -> Self {
        Rectangle::new(
            storage[0].round() as i32,
            storage[1].round() as i32,
            storage[2].round() as i32,
            storage[3].round() as i32,
        )
    }
}

pub trait FloatArray: PartialEq + Clone + Copy {
    fn as_float_slice(&mut self) -> &mut [f32];
}

impl<const N: usize> FloatArray for [f32; N] {
    fn as_float_slice(&mut self) -> &mut [f32] {
        self
    }
}
