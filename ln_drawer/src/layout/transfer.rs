use ln_world::{Handle, World};

use crate::layout::val::{Val, ValChanged};

pub struct Transfer<'w, T: Send + 'static, U: Send + 'static> {
    world: &'w World,
    this: Handle<Val<T>>,
    that: Handle<Val<U>>,
}

impl<T: Send + 'static, U: Send + 'static> Transfer<'_, T, U> {
    pub fn new(world: &World, this: Handle<Val<T>>, that: Handle<Val<U>>) -> Transfer<'_, T, U> {
        Transfer { world, this, that }
    }

    pub fn build(&self, f: impl Fn(&T) -> U + Send + 'static) -> Handle<Val<U>> {
        self.build_with(move |i, _| f(i))
    }

    pub fn build_with(&self, f: impl Fn(&T, &World) -> U + Send + 'static) -> Handle<Val<U>> {
        let &Transfer { world, this, that } = self;
        world.queue(move |world| {
            {
                // init
                let this = world.fetch(this);
                let mut that = world.fetch_mut(that);
                that.val = f(&this.val, world);
            }

            world.observer(this, move |ValChanged, world| {
                let this = world.fetch(this);
                let mut that = world.fetch_mut(that);
                that.val = f(&this.val, world);
                world.queue_trigger(that.handle(), ValChanged);
            });
        });
        that
    }
}

// TODO remove Default requirement after ln_world pre-allocate function
impl<T: Send + 'static, U: Send + Default + 'static> Transfer<'_, T, U> {
    pub fn derive(world: &World, this: Handle<Val<T>>) -> Transfer<'_, T, U> {
        let that = world.insert(Val::new(U::default()));
        Transfer { world, this, that }
    }
}

impl<T: Send + Clone + Copy + 'static> Transfer<'_, T, T> {
    pub fn copy(&self) -> Handle<Val<T>> {
        self.build(|&x| x)
    }
}
