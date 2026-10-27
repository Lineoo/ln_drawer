use ln_world::{Handle, World};

use crate::layout::val::{Val, ValChanged};

/// This is for single-to-single data-bind. See [`Packer`] for more complicated binding.
pub struct Transfer<'w, T: Send + 'static, U: Send + 'static> {
    world: &'w World,
    this: Handle<Val<T>>,
    that: Option<Handle<Val<U>>>,
}

impl<T: Send + 'static, U: Send + 'static> Transfer<'_, T, U> {
    pub fn new(world: &World, this: Handle<Val<T>>, that: Handle<Val<U>>) -> Transfer<'_, T, U> {
        Transfer {
            world,
            this,
            that: Some(that),
        }
    }

    pub fn derive(world: &World, this: Handle<Val<T>>) -> Transfer<'_, T, U> {
        Transfer {
            world,
            this,
            that: None,
        }
    }

    pub fn build(&self, f: impl Fn(&T) -> U + Send + 'static) -> Handle<Val<U>> {
        self.build_with(move |i, _| f(i))
    }

    pub fn build_with(&self, f: impl Fn(&T, &World) -> U + Send + 'static) -> Handle<Val<U>> {
        let &Transfer { world, this, that } = self;
        let fun = world.insert(Val::new(f));
        let that = if let Some(that) = that {
            world.queue(move |world| {
                let this = world.fetch(this);
                let fun = world.fetch(fun);
                let mut that = world.fetch_mut(that);
                that.val = (fun.val)(&this.val, world);
            });
            that
        } else {
            world.insert_with(move |world| {
                let this = world.fetch(this);
                let fun = world.fetch(fun);
                Val::new((fun.val)(&this.val, world))
            })
        };
        world.queue(move |world| {
            world.observer(this, move |ValChanged, world| {
                let this = world.fetch(this);
                let fun = world.fetch(fun);
                let mut that = world.fetch_mut(that);
                that.val = (fun.val)(&this.val, world);
                world.queue_trigger(that.handle(), ValChanged);
            });
        });
        that
    }
}

impl<T: Send + Clone + Copy + 'static> Transfer<'_, T, T> {
    pub fn copy(&self) -> Handle<Val<T>> {
        self.build(|&x| x)
    }
}
