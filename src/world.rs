use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

// Element Definition //

/// A shared form of objects in the [`World`].
pub trait Element: Any {
    #[expect(unused_variables)]
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {}
}

/// A way to build [`Element`] in the [`World`].
pub trait ElementDescriptor {
    type Target;
    fn build(self, world: &World) -> Self::Target;
}

// Handle Definition //

/// Represent an element in the [`World`]. It may not be valid.
pub struct Handle<T: ?Sized = dyn Any>(usize, PhantomData<T>);

impl<T: ?Sized> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Handle<T> {}

impl<T: ?Sized> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T: ?Sized> Eq for Handle<T> {}

impl<T: ?Sized> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: ?Sized> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", type_name::<T>(), self.0)
    }
}

impl<T: ?Sized> fmt::Display for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl<T: Element> From<Handle<T>> for Handle {
    fn from(value: Handle<T>) -> Self {
        Handle(value.0, PhantomData)
    }
}

impl<T: Element> Handle<T> {
    pub fn untyped(self) -> Handle<dyn Any> {
        self.cast()
    }
}

impl<T: ?Sized> Handle<T> {
    fn cast<U: ?Sized>(self) -> Handle<U> {
        Handle(self.0, PhantomData)
    }
}

// World Management //

// Center of multiple accesses in world, which also prevents constructional changes
pub struct World {
    storage: WorldStorage,
    occupied: RefCell<HashMap<Handle, isize>>,
    cell_idx: RefCell<Handle>,
    inserted: RefCell<HashSet<Handle>>,
    removed: RefCell<HashSet<Handle>>,
    #[expect(clippy::type_complexity)]
    queue: RefCell<Vec<Box<dyn FnOnce(&mut World)>>>,
}

pub struct WorldStorage {
    curr_idx: Handle,
    elements: HashMap<Handle, Box<dyn Any>>,
    cache: HashMap<TypeId, HashSet<Handle>>,
}

impl Default for World {
    fn default() -> Self {
        World {
            storage: WorldStorage::default(),
            occupied: RefCell::new(HashMap::new()),
            cell_idx: RefCell::new(Handle(0, PhantomData)),
            inserted: RefCell::default(),
            removed: RefCell::default(),
            queue: RefCell::default(),
        }
    }
}

impl Default for WorldStorage {
    fn default() -> Self {
        WorldStorage {
            curr_idx: Handle(0, PhantomData),
            elements: HashMap::new(),
            cache: HashMap::new(),
        }
    }
}

impl World {
    // lifecycle //

    pub fn build<B: ElementDescriptor<Target: Element>>(&self, descriptor: B) -> Handle<B::Target> {
        let element = descriptor.build(self);
        self.insert(element)
    }

    /// Due to limit of cell, the inserted element cannot be fetched until `flush` is called.
    /// One exception is entry, which can still be used normally.
    pub fn insert<T: Element>(&self, element: T) -> Handle<T> {
        // get estimate_handle
        // cell-mode insertion depends on *retained* handle
        let mut cell_idx = self.cell_idx.borrow_mut();
        let estimate_handle = cell_idx.cast::<T>();
        cell_idx.0 += 1;
        log::trace!("insert: {:?}", estimate_handle);

        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(estimate_handle.cast());

        self.queue(move |world| {
            world
                .storage
                .elements
                .insert(estimate_handle.cast(), Box::new(element));

            // update cache
            let cache = world.storage.cache.entry(TypeId::of::<T>()).or_default();
            cache.insert(estimate_handle.cast());

            // when_inserted
            let mut element = world.fetch_mut(estimate_handle).unwrap();
            element.when_inserted(world, estimate_handle);
            drop(element);
        });

        estimate_handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the owned value of removed element.
    /// Notice that this removal actually ignore the borrow check so you can still preserve the reference if you have
    /// fetched it before invoking remove.
    pub fn remove<T: ?Sized>(&self, handle: Handle<T>) -> usize {
        let handle = handle.cast();

        if !self.validate(handle) {
            return 0;
        }

        let type_id = (**self.storage.elements.get(&handle).unwrap()).type_id();
        log::trace!("remove {:?}", handle);

        let mut cnt = 1;

        // maintain dependency
        if let Some(mut dependencies) = self.single_fetch_mut::<Dependencies>()
            && let Some(this) = dependencies.0.remove(&handle)
        {
            // clean for parents
            for depend_on in this.depend_on {
                let Some(depend_on) = dependencies.0.get_mut(&depend_on) else {
                    continue;
                };

                // search for itself and swap remove
                for i in 0..depend_on.depend_by.len() {
                    if depend_on.depend_by[i] == handle {
                        depend_on.depend_by.swap_remove(i);
                        break;
                    }
                }
            }

            // remove children
            drop(dependencies);
            for child in this.depend_by {
                cnt += self.remove(child);
            }
        }

        // prevent element from being fetch again
        let mut removed = self.removed.borrow_mut();
        removed.insert(handle);
        drop(removed);

        // trigger lifecycle events
        self.trigger(handle, &Destroy);

        self.queue(move |world| {
            // update cache
            let cache = world.storage.cache.entry(type_id).or_default();
            cache.remove(&handle);

            world.storage.elements.remove(&handle);
        });

        cnt
    }

    /// Insertion without `flush` will not be included
    pub fn validate<T: ?Sized>(&self, handle: Handle<T>) -> bool {
        if self.removed.borrow().contains(&handle.cast()) {
            return false;
        }

        self.storage.elements.contains_key(&handle.cast())
    }

    // cell-mode ops //

    /// Check whether target element can be borrowed immutably
    pub fn occupied<T: ?Sized>(&self, handle: Handle<T>) -> bool {
        let occupied = self.occupied.borrow();
        occupied.get(&handle.cast()).is_some_and(|cnt| *cnt < 0)
    }

    /// Check whether target element can be borrowed mutably
    pub fn occupied_mut<T: ?Sized>(&self, handle: Handle<T>) -> bool {
        let occupied = self.occupied.borrow();
        occupied.get(&handle.cast()).is_some_and(|cnt| *cnt != 0)
    }

    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let mut queue = self.queue.borrow_mut();
        queue.push(Box::new(f));
    }

    pub fn flush(&mut self) {
        self.storage.curr_idx = *self.cell_idx.get_mut();

        let queue = self.queue.get_mut();
        let mut buf = Vec::with_capacity(queue.len());
        buf.append(queue);

        for cmd in buf {
            cmd(self);
            self.flush();
        }
    }

    // fetch //

    pub fn fetch<T: Element>(&self, handle: Handle<T>) -> Option<Ref<'_, T>> {
        let handle = handle.cast();

        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.storage.elements.get(&handle)?.downcast_ref()?;

        Some(Ref {
            ptr: element as *const T,
            world: self,
            handle,
        })
    }

    pub fn fetch_dyn<T: ?Sized>(&self, handle: Handle<T>) -> Option<Ref<'_>> {
        let handle = handle.cast();

        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.storage.elements.get(&handle)?.as_ref();

        Some(Ref {
            ptr: element as *const dyn Any,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: Handle<T>) -> Option<RefMut<'_, T>> {
        let handle = handle.cast();

        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.storage.elements.get(&handle)?.downcast_ref()?;

        Some(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut_dyn<T: ?Sized>(&self, handle: Handle<T>) -> Option<RefMut<'_>> {
        let handle = handle.cast();

        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.storage.elements.get(&handle)?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Any as *mut dyn Any,
            world: self,
            handle,
        })
    }

    // singleton //

    pub fn single<T: Element>(&self) -> Option<Handle<T>> {
        let removed = self.removed.borrow();
        let cache = self.storage.cache.get(&TypeId::of::<T>())?;
        let mut iter = cache.iter().filter(|&x| !removed.contains(x));
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(ret.cast())
    }

    pub fn single_fetch<T: Element>(&self) -> Option<Ref<'_, T>> {
        self.fetch(self.single::<T>()?)
    }

    pub fn single_fetch_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        self.fetch_mut(self.single::<T>()?)
    }

    // iteration //

    pub fn foreach<T: Element>(&self, mut f: impl FnMut(Handle<T>)) {
        let removed = self.removed.borrow();
        let Some(cache) = self.storage.cache.get(&TypeId::of::<T>()) else {
            return;
        };

        for handle in cache.iter().filter(|&x| !removed.contains(x)) {
            f(handle.cast());
        }
    }

    pub fn foreach_fetch<T: Element>(&self, mut f: impl FnMut(Handle<T>, Ref<T>)) {
        self.foreach::<T>(|handle| f(handle, self.fetch(handle).unwrap()))
    }

    pub fn foreach_fetch_mut<T: Element>(&self, mut f: impl FnMut(Handle<T>, RefMut<T>)) {
        self.foreach::<T>(|handle| f(handle, self.fetch_mut(handle).unwrap()))
    }

    // observer & trigger //

    pub fn observer<T: ?Sized + 'static, E: 'static>(
        &self,
        target: Handle<T>,
        mut action: impl FnMut(&E, &World, Handle<T>) + 'static,
    ) -> Handle {
        let handle = self.insert(Observer {
            action: Box::new(move |event, world| {
                action(event, world, target);
            }),
            target: target.cast(),
        });

        handle.cast()
    }

    pub fn trigger<T: ?Sized + 'static, E: 'static>(&self, target: Handle<T>, event: E) {
        self.queue(move |world| {
            if let Some(observers) = world.single_fetch::<Observers<E>>()
                && let Some(observers) = observers.members.get(&target.cast())
            {
                for mut observer in observers.iter().filter_map(|x| world.fetch_mut(*x)) {
                    (observer.action)(&event, world);
                }
            }
        })
    }

    // dependency //

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn dependency<T: ?Sized, U: ?Sized>(&self, target: Handle<T>, depend_on: Handle<U>) {
        let target = target.cast();
        let depend_on = depend_on.cast();

        if !self.validate(depend_on) {
            log::error!("{target:?} try to depend on {depend_on:?}, which does not exist");
            return;
        }

        match self.single_fetch_mut::<Dependencies>() {
            Some(mut dependencies) => {
                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(target);
                let depend = dependencies.0.entry(target).or_default();
                depend.depend_on.push(depend_on);
            }
            None => {
                let mut dependencies = Dependencies::default();

                log::debug!("init dependencies");

                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(target);
                let depend = dependencies.0.entry(target).or_default();
                depend.depend_on.push(depend_on);
                self.insert(dependencies);
            }
        }
    }
}

/// A world's immutable element reference.
pub struct Ref<'world, T: ?Sized = dyn Any> {
    ptr: *const T,
    world: &'world World,
    handle: Handle,
}

/// A world's limitedly mutable element reference.
pub struct RefMut<'world, T: ?Sized = dyn Any> {
    ptr: *mut T,
    world: &'world World,
    handle: Handle,
}

impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}

impl<T: ?Sized> Deref for RefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}

impl<T: ?Sized> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_mut().unwrap() }
    }
}

impl<T: ?Sized> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.borrow_mut();
        let cnt = occupied
            .get_mut(&{
                let this = self.handle;
                this.cast()
            })
            .unwrap();
        *cnt -= 1;
    }
}

impl<T: ?Sized> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.borrow_mut();
        let cnt = occupied
            .get_mut(&{
                let this = self.handle;
                this.cast()
            })
            .unwrap();
        *cnt += 1;
    }
}

// Observer & Trigger //
// FIXME observer cleanup

#[derive(Default)]
struct Observers<E> {
    members: HashMap<Handle, SmallVec<[Handle<Observer<E>>; 1]>>,
}

#[expect(clippy::type_complexity)]
struct Observer<E> {
    action: Box<dyn FnMut(&E, &World)>,
    target: Handle,
}

impl<E: 'static> Element for Observers<E> {}

impl<E: 'static> Element for Observer<E> {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        match world.single_fetch_mut::<Observers<E>>() {
            Some(mut observers) => {
                let observers = observers.members.entry(self.target).or_default();
                observers.push(this);
            }
            None => {
                let mut observers = Observers::<E> {
                    members: HashMap::new(),
                };

                log::debug!("register events: {}", type_name::<E>());

                let observer = observers.members.entry(self.target).or_default();
                observer.push(this);
                world.insert(observers);
            }
        }

        world.dependency(this, self.target);
    }
}

// Dependency //

#[derive(Default)]
struct Dependencies(HashMap<Handle, Dependency>);

#[derive(Default)]
struct Dependency {
    depend_on: SmallVec<[Handle; 1]>,
    depend_by: SmallVec<[Handle; 4]>,
}

impl Element for Dependencies {}

// Attaches //

#[derive(Default)]
struct Attaches<T: Element>(HashMap<Handle, Handle<T>>);

impl<T: Element> Element for Attaches<T> {}

// Lifecycle Events //

pub struct Destroy;

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestInserter(usize);
    impl Element for TestInserter {}

    #[derive(Debug, PartialEq, Eq)]
    struct TestGoodInserter(usize);
    impl Element for TestGoodInserter {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestEvent(usize);

    #[test]
    fn basic() {
        let mut world = World::default();

        let tester1h = world.insert(TestInserter(0xFC01));
        let tester2h = world.insert(TestInserter(0xFF02));
        let tester3h = world.insert(TestInserter(0xFB03));

        world.flush();

        let mut tester1 = world.fetch_mut(tester1h).unwrap();
        let mut tester2 = world.fetch_mut(tester2h).unwrap();
        let tester3 = world.fetch(tester3h).unwrap();

        tester2.0 = 0xCC02;
        tester1.0 = tester3.0;

        drop(tester1);
        drop(tester2);
        drop(tester3);

        world.remove(tester3h);

        assert!(!world.validate(tester3h));
        assert_eq!(world.fetch(tester1h).unwrap().0, 0xFB03);
        assert_eq!(world.fetch(tester2h).unwrap().0, 0xCC02);
    }

    #[test]
    #[should_panic = "is mutably borrowed"]
    fn runtime_borrow_panic() {
        let mut world = World::default();
        let tester1h = world.insert(TestInserter(0xFC01));
        world.flush();

        let _inserter1 = world.fetch_mut(tester1h).unwrap();
        let _inserter2 = world.fetch(tester1h).unwrap();
    }

    #[test]
    fn runtime_borrow_conflict() {
        let mut world = World::default();
        let tester1h = world.insert(TestInserter(0xFC01));
        world.flush();

        {
            assert!(!world.occupied(tester1h));
            assert!(!world.occupied_mut(tester1h));
        }

        {
            let _inserter1 = world.fetch_mut(tester1h).unwrap();

            assert!(world.occupied(tester1h));
            assert!(world.occupied_mut(tester1h));
        }

        {
            let _inserter1 = world.fetch(tester1h).unwrap();

            assert!(!world.occupied(tester1h));
            assert!(world.occupied_mut(tester1h));
        }
    }

    #[test]
    fn loop_dependency() {
        let mut world = World::default();

        let left = world.insert(TestInserter(1));
        let right = world.insert(TestGoodInserter(2));
        let right_now = world.insert(TestGoodInserter(3));
        let but = world.insert(TestInserter(4));

        world.flush();

        world.dependency(left, right);
        world.dependency(right, left);
        world.dependency(right_now, right);

        world.remove(left);

        world.flush();

        assert!(!world.validate(left));
        assert!(!world.validate(right));
        assert!(!world.validate(right_now));
        assert!(world.validate(but));
    }

    #[test]
    fn observers() {
        let mut world = World::default();

        let left = world.insert(TestInserter(1));
        let right = world.insert(TestGoodInserter(2));

        world.flush();

        world.observer(left, |TestEvent(i), world, this| {
            let mut this = world.fetch_mut(this).unwrap();
            this.0 += i;
        });

        let obs = world.observer(left, move |TestEvent(i), world, _| {
            let mut this = world.fetch_mut(right).unwrap();
            this.0 += i;
        });

        world.dependency(obs, right);

        world.trigger(left, TestEvent(10));

        world.flush();

        assert_eq!(&*world.fetch(left).unwrap(), &TestInserter(11));
        assert_eq!(&*world.fetch(right).unwrap(), &TestGoodInserter(12));
    }
}
