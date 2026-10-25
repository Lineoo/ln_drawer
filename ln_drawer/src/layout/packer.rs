use ln_world::{Handle, HandleGeneric, World};

use crate::layout::val::{Val, ValChanged};

pub struct Packer<'w, T: Send + 'static, F: Send + 'static> {
    world: &'w World,
    that: Handle<Val<T>>,
    fun: Handle<Val<F>>,
}

impl<T: Send + 'static, F: Fn(&World) -> T + Send + 'static> Packer<'_, T, F> {
    pub fn new(world: &World, that: Handle<Val<T>>, fun: F) -> Packer<'_, T, F> {
        let fun = world.insert(Val::new(fun));
        world.queue(move |world| {
            let fun = world.fetch(fun);
            let mut that = world.fetch_mut(that);
            that.val = (fun.val)(world);
            world.queue_trigger(that.handle(), ValChanged);
        });
        Packer { world, that, fun }
    }

    pub fn listen(&self, this: impl HandleGeneric) -> &Self {
        let &Packer { world, that, fun } = self;
        world.observer(this, move |ValChanged, world| {
            let fun = world.fetch(fun);
            let mut that = world.fetch_mut(that);
            that.val = (fun.val)(world);
            world.queue_trigger(that.handle(), ValChanged);
        });
        self
    }

    pub fn finish(&self) -> Handle<Val<T>> {
        self.that
    }
}

// TODO remove Default requirement after ln_world pre-allocate function
impl<T: Send + Default + 'static, F: Fn(&World) -> T + Send + 'static> Packer<'_, T, F> {
    pub fn derive(world: &World, fun: F) -> Packer<'_, T, F> {
        let that = world.insert(Val::new(T::default()));
        Packer::new(world, that, fun)
    }
}
