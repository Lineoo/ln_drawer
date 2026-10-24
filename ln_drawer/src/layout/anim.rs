use std::time::{Duration, Instant};

use glam::Vec4;
use ln_world::{Descriptor, Element, Handle, World};

use crate::{
    layout::val::{Val, ValChanged},
    lnwin::Lnwindow,
    measures::Rectangle,
    render::{RenderInformation, RenderPrepareControl},
};

pub struct Animator<T: AnimationType> {
    pub target: Handle<Val<T>>,
    pub factor: f32,
}

pub struct AnimationCore<T: AnimationType> {
    pub src: Handle<Val<T>>,
    pub dst: Handle<Val<T>>,
    pub src_st: T::Storage,
    pub dst_st: T::Storage,
    pub factor: f32,

    last_update: Instant,
}

impl<T: AnimationType> Animator<T> {
    pub fn new(target: Handle<Val<T>>, factor: f32) -> Self {
        Self { target, factor }
    }
}

impl<T: AnimationType + Default> Descriptor for Animator<T> {
    type Target = Handle<Val<T>>;

    fn when_build(self, world: &World) -> Self::Target {
        let result = world.insert(Val::new(T::default()));
        world.queue(move |world| {
            let value = world.fetch(self.target).val;
            let mut result = world.fetch_mut(result);
            result.val = value;
            world.insert(AnimationCore {
                src: result.handle(),
                dst: self.target,
                src_st: value.into_storage(),
                dst_st: value.into_storage(),
                factor: self.factor,
                last_update: Instant::now(),
            });
        });
        result
    }
}

impl<T: AnimationType> Element for AnimationCore<T> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let source = self.dst;
        let control = world.insert(RenderPrepareControl {
            prepare: Box::new(move |world| {
                let this = &mut *world.fetch_mut(this);
                let mut src = world.fetch_mut(this.src);

                // calculate next value

                let now = Instant::now();
                let changed = step::<T>(
                    &mut this.src_st,
                    &mut this.dst_st,
                    this.factor,
                    now - this.last_update,
                );
                this.last_update = now;

                // send event

                let new = T::from_storage(this.src_st);
                let refresh = changed || new != src.val;
                if refresh {
                    src.val = new;
                    world.queue_trigger(this.src, ValChanged);
                }

                Some(RenderInformation {
                    keep_redrawing: refresh,
                })
            }),
        });

        world.observer(source, move |ValChanged, world| {
            let mut this = world.fetch_mut(this);
            let dst = world.fetch(this.dst);
            this.dst_st = T::into_storage(dst.val);
            this.last_update = Instant::now();
            world.single_fetch::<Lnwindow>().window.request_redraw();
        });

        world.dependency(control, this);
    }
}

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

pub trait AnimationType: PartialEq + Clone + Copy + Send + 'static {
    type Storage: FloatArray + Send;
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

impl AnimationType for Vec4 {
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
