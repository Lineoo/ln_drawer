use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::mpsc::{Receiver, Sender, channel},
};

use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

// Definition //

/// A shared form of objects in the [`World`].
pub trait Element: Any {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let _ = (world, this);
    }

    fn when_modify(&mut self, world: &World, this: Handle<Self>) {
        let _ = (world, this);
    }

    fn when_remove(&mut self, world: &World, this: Handle<Self>) {
        let _ = (world, this);
    }
}

/// A way to setup in the [`World`].
pub trait Descriptor {
    type Target;
    fn when_build(self, world: &World) -> Self::Target;
}

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

impl<T: Element> From<Handle<T>> for Handle<dyn Any> {
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

/// Handle with debug information.
#[derive(Clone, Copy)]
pub struct HandleInfo(Handle, &'static str);

impl fmt::Debug for HandleInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", self.1, self.0)
    }
}

impl fmt::Display for HandleInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl<T: ?Sized> From<Handle<T>> for HandleInfo {
    fn from(value: Handle<T>) -> Self {
        HandleInfo(value.cast(), type_name::<T>())
    }
}

// World Management //

// Center of multiple accesses in world, which also prevents constructional changes
pub struct World {
    cell_idx: RefCell<Handle>,

    members: HashMap<Handle, Box<dyn Any>>,
    typehint: HashMap<TypeId, WorldType>,

    occupied: RefCell<HashMap<Handle, isize>>,
    inserted: RefCell<HashSet<Handle>>,
    removed: RefCell<HashSet<Handle>>,

    queue: Receiver<WorldCommand>,
    commander: Sender<WorldCommand>,
}

struct WorldType {
    cache: HashSet<Handle>,
    type_name: &'static str,
    when_insert: fn(&mut dyn Any, &World, Handle),
    when_modify: fn(&mut dyn Any, &World, Handle),
    when_remove: fn(&mut dyn Any, &World, Handle),
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("{0:?} was just inserted")]
    JustInserted(HandleInfo),

    #[error("{0:?} was just removed")]
    JustRemoved(HandleInfo),

    #[error("{0:?} does not exist")]
    InvalidHandle(HandleInfo),

    #[error("{0:?} try to depend on {1:?}, which does not exist")]
    ToxicDependency(HandleInfo, HandleInfo),

    #[error("{0:?} has wrong type")]
    UnmatchedType(HandleInfo),

    #[error("{0:?} is mutably borrowed")]
    Unavailable(HandleInfo),

    #[error("{0:?} is borrowed")]
    UnavailableMut(HandleInfo),

    #[error("{0} is not singleton (0 in total)")]
    SingletonNoSuch(&'static str),

    #[error("{0} is not singleton ({1} in total)")]
    SingletonTooMany(&'static str, usize),
}

impl Default for World {
    fn default() -> Self {
        let (commander, queue) = channel();
        World {
            cell_idx: RefCell::new(Handle(0, PhantomData)),
            members: HashMap::default(),
            typehint: HashMap::default(),
            occupied: RefCell::default(),
            inserted: RefCell::default(),
            removed: RefCell::default(),
            queue,
            commander,
        }
    }
}

impl World {
    /// Will access data from world to build target object.
    pub fn build<B: Descriptor>(&self, descriptor: B) -> B::Target {
        descriptor.when_build(self)
    }

    /// Due to limit of cell, the inserted element cannot be fetched until `flush` is called.
    /// Meanwhile, handle-based ops, like `observer` or `dependency`, can still be used normally.
    pub fn insert<T: Element>(&self, element: T) -> Handle<T> {
        // assign estimate handle
        let mut cell_idx = self.cell_idx.borrow_mut();
        let handle = cell_idx.cast::<T>();
        cell_idx.0 += 1;

        // write immediate record
        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(handle.cast());

        // delay execution
        self.queue(move |world| {
            // get type table ready
            let typehint = world.typehint.entry(TypeId::of::<T>()).or_insert_with(|| {
                log::debug!("register elements: {}", type_name::<T>());
                WorldType {
                    cache: HashSet::new(),
                    type_name: type_name::<T>(),
                    when_insert: |elem, world, handle| {
                        T::when_insert(elem.downcast_mut().unwrap(), world, handle.cast());
                    },
                    when_modify: |elem, world, handle| {
                        T::when_modify(elem.downcast_mut().unwrap(), world, handle.cast());
                    },
                    when_remove: |elem, world, handle| {
                        T::when_remove(elem.downcast_mut().unwrap(), world, handle.cast());
                    },
                }
            });

            // push into storage
            world.members.insert(handle.cast(), Box::new(element));

            // update cache
            typehint.cache.insert(handle.cast());

            // when_insert
            let mut element = world.fetch_mut(handle).unwrap();
            element.when_insert(world, handle);

            log::trace!("insert: {:?}", handle);
        });

        handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the owned value of removed element.
    pub fn remove<T: ?Sized + 'static>(&self, handle: Handle<T>) -> Result<usize, WorldError> {
        let handle_any = handle.cast();

        self.available_mut(handle)?;

        // when_remove
        // SAFETY: we have checked the mutability
        let element = self.members.get(&handle_any).unwrap().as_ref();
        let when_remove = self.typehint.get(&element.type_id()).unwrap().when_remove;
        let element = element as *const dyn Any as *mut dyn Any;
        when_remove(unsafe { element.as_mut().unwrap() }, self, handle_any);

        // maintain dependency
        let mut cnt = 1;
        if let Ok(mut dependencies) = self.single_fetch_mut::<Dependencies>()
            && let Some(this) = dependencies.0.remove(&handle_any)
        {
            // clean for parents
            for depend_on in this.depend_on {
                let Some(depend_on) = dependencies.0.get_mut(&depend_on) else {
                    continue;
                };

                // search for itself and swap remove
                for i in 0..depend_on.depend_by.len() {
                    if depend_on.depend_by[i] == handle_any {
                        depend_on.depend_by.swap_remove(i);
                        break;
                    }
                }
            }

            // remove children
            drop(dependencies);
            for child in this.depend_by {
                cnt += self.remove(child)?;
            }
        }

        // write immediate record
        let mut removed = self.removed.borrow_mut();
        removed.insert(handle_any);
        drop(removed);

        self.queue(move |world| {
            // update cache
            let type_id = world.members.get(&handle_any).unwrap().as_ref().type_id();
            let typehint = world.typehint.get_mut(&type_id).unwrap();
            typehint.cache.remove(&handle_any);

            // pop out storage
            world.members.remove(&handle_any);

            log::trace!("remove {:?}", handle);
        });

        Ok(cnt)
    }

    // cell-mode ops //

    /// Check whether target element exists, insertion without `flush` will *NOT* be included.
    pub fn validate<T: ?Sized>(&self, handle: Handle<T>) -> Result<(), WorldError> {
        if self.removed.borrow().contains(&handle.cast()) {
            return Err(WorldError::JustRemoved(handle.into()));
        }

        if !self.members.contains_key(&handle.cast()) {
            if self.inserted.borrow().contains(&handle.cast()) {
                return Err(WorldError::JustInserted(handle.into()));
            }

            return Err(WorldError::InvalidHandle(handle.into()));
        }

        Ok(())
    }

    /// Check whether target element can be borrowed immutably, insertion without
    /// `flush` will *NOT* be included.
    pub fn available<T: ?Sized>(&self, handle: Handle<T>) -> Result<(), WorldError> {
        self.validate(handle)?;

        let occupied = self.occupied.borrow();
        if occupied.get(&handle.cast()).is_some_and(|cnt| *cnt < 0) {
            panic!("{}", WorldError::Unavailable(handle.into()));
        }

        Ok(())
    }

    /// Check whether target element can be borrowed mutably, insertion without
    /// `flush` will *NOT* be included.
    pub fn available_mut<T: ?Sized>(&self, handle: Handle<T>) -> Result<(), WorldError> {
        self.validate(handle)?;

        let occupied = self.occupied.borrow();
        if occupied.get(&handle.cast()).is_some_and(|cnt| *cnt != 0) {
            panic!("{}", WorldError::UnavailableMut(handle.into()));
        }

        Ok(())
    }

    pub fn commander(&self) -> Commander {
        Commander {
            inner: self.commander.clone(),
        }
    }

    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let result = self.commander.send(Box::new(f));
        if let Err(err) = result {
            log::error!("error in world queue ops: {err}");
        }
    }

    pub fn flush(&mut self) {
        let buf = self.queue.try_iter().collect::<Vec<_>>();
        for cmd in buf {
            cmd(self);
            self.flush();
        }
    }

    // fetch //

    pub fn fetch<T: Element>(&self, handle: Handle<T>) -> Result<Ref<'_, T>, WorldError> {
        self.available(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() += 1;

        let element = (self.members)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(handle.into()))?
            .downcast_ref()
            .ok_or(WorldError::UnmatchedType(handle.into()))?;

        Ok(Ref {
            ptr: element as *const T,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: Handle<T>) -> Result<RefMut<'_, T>, WorldError> {
        self.available_mut(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() -= 1;

        let element = (self.members)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(handle.into()))?
            .downcast_ref()
            .ok_or(WorldError::UnmatchedType(handle.into()))?;

        Ok(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
            modified: false,
        })
    }

    // singleton //

    pub fn single<T: Element>(&self) -> Result<Handle<T>, WorldError> {
        let cache = (self.typehint)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::SingletonNoSuch(type_name::<T>()))?;

        let removed = self.removed.borrow();
        let mut iter = cache.cache.iter().filter(|&x| !removed.contains(x));

        let ret = iter
            .next()
            .ok_or(WorldError::SingletonNoSuch(type_name::<T>()))?;

        if iter.next().is_some() {
            let mut cnt = 2;
            for _ in iter {
                cnt += 1;
            }

            return Err(WorldError::SingletonTooMany(type_name::<T>(), cnt));
        }

        Ok(ret.cast())
    }

    pub fn single_fetch<T: Element>(&self) -> Result<Ref<'_, T>, WorldError> {
        self.fetch(self.single::<T>()?)
    }

    pub fn single_fetch_mut<T: Element>(&self) -> Result<RefMut<'_, T>, WorldError> {
        self.fetch_mut(self.single::<T>()?)
    }

    // iteration //

    pub fn len<T: Element>(&self) -> usize {
        (self.typehint.get(&TypeId::of::<T>()))
            .map(|x| x.cache.len())
            .unwrap_or_default()
    }

    pub fn foreach<T: Element>(&self, mut f: impl FnMut(Handle<T>)) {
        let Some(cache) = self.typehint.get(&TypeId::of::<T>()) else {
            return;
        };

        for handle in cache.cache.iter() {
            let removed = self.removed.borrow();
            if removed.contains(handle) {
                continue;
            }

            drop(removed);

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
            action: Box::new(move |event, world| action(event, world, target)),
            target: target.cast(),
        });

        handle.cast()
    }

    /// Will immediately triggered and acquire mutable access to `target`.
    pub fn trigger<T: ?Sized + 'static, E: 'static>(&self, target: Handle<T>, event: &E) -> usize {
        let mut cnt = 0;
        if let Ok(observers) = self.single_fetch::<Observers<E>>()
            && let Some(observers) = observers.members.get(&target.cast())
        {
            for mut observer in observers.iter().filter_map(|x| self.fetch_mut(*x).ok()) {
                (observer.action)(event, self);
                cnt += 1;
            }
        }

        cnt
    }

    // dependency //

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn dependency<T: ?Sized, U: ?Sized>(&self, target: Handle<T>, depend_on: Handle<U>) {
        if self.removed.borrow().contains(&depend_on.cast())
            || (!self.members.contains_key(&depend_on.cast())
                && !self.inserted.borrow().contains(&depend_on.cast()))
        {
            let err = WorldError::ToxicDependency(target.into(), depend_on.into());
            log::error!("{err:?}");
            return;
        }

        let target = target.cast();
        let depend_on = depend_on.cast();

        match self.single_fetch_mut::<Dependencies>() {
            Ok(mut dependencies) => {
                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(target);
                let depend = dependencies.0.entry(target).or_default();
                depend.depend_on.push(depend_on);
            }
            Err(WorldError::SingletonNoSuch(_)) => {
                let mut dependencies = Dependencies::default();

                log::debug!("init dependencies");

                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(target);
                let depend = dependencies.0.entry(target).or_default();
                depend.depend_on.push(depend_on);
                self.insert(dependencies);
            }
            Err(err) => {
                todo!("{err}");
            }
        }
    }
}

/// A world's immutable element reference.
pub struct Ref<'world, T: Element> {
    ptr: *const T,
    world: &'world World,
    handle: Handle<T>,
}

/// A world's limitedly mutable element reference.
pub struct RefMut<'world, T: Element> {
    ptr: *mut T,
    world: &'world World,
    handle: Handle<T>,
    modified: bool,
}

impl<T: Element> Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}

impl<T: Element> Deref for RefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}

impl<T: Element> DerefMut for RefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.modified = true;

        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_mut().unwrap() }
    }
}

impl<T: Element> Drop for Ref<'_, T> {
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

impl<T: Element> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        if self.modified {
            T::when_modify(
                // SAFETY: still inside the RefMut's guarantee
                unsafe { self.ptr.as_mut().unwrap() },
                self.world,
                self.handle,
            );
        }

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

impl<T: Element> Ref<'_, T> {
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }
}

impl<T: Element> RefMut<'_, T> {
    pub fn handle(&self) -> Handle<T> {
        self.handle
    }

    pub fn modified(&mut self) {
        self.modified = true;
    }
}

// Commander //

type WorldCommand = Box<dyn FnOnce(&mut World)>;

/// A flexible command access to world.
#[derive(Debug, Clone)]
pub struct Commander {
    inner: Sender<WorldCommand>,
}

impl Commander {
    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let result = self.inner.send(Box::new(f));
        if let Err(err) = result {
            log::error!("error in world queue ops: {err}");
        }
    }
}

// Observer & Trigger //

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
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        match world.single_fetch_mut::<Observers<E>>() {
            Ok(mut observers) => {
                let observers = observers.members.entry(self.target).or_default();
                observers.push(this);
            }
            Err(WorldError::SingletonNoSuch(_)) => {
                let mut observers = Observers::<E> {
                    members: HashMap::new(),
                };

                log::debug!("register events: {}", type_name::<E>());

                let observer = observers.members.entry(self.target).or_default();
                observer.push(this);
                world.insert(observers);
            }
            Err(err) => {
                todo!("{err}")
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

        world.remove(tester3h).unwrap();

        assert!(world.validate(tester3h).is_err());
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
            assert!(world.available(tester1h).is_ok());
            assert!(world.available_mut(tester1h).is_ok());
        }

        {
            let _inserter1 = world.fetch_mut(tester1h).unwrap();

            assert!(world.available(tester1h).is_err());
            assert!(world.available_mut(tester1h).is_err());
        }

        {
            let _inserter1 = world.fetch(tester1h).unwrap();

            assert!(world.available(tester1h).is_ok());
            assert!(world.available_mut(tester1h).is_err());
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

        world.remove(left).unwrap();

        world.flush();

        assert!(world.validate(left).is_err());
        assert!(world.validate(right).is_err());
        assert!(world.validate(right_now).is_err());
        assert!(world.validate(but).is_ok());
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

        world.trigger(left, &TestEvent(10));

        world.flush();

        assert_eq!(&*world.fetch(left).unwrap(), &TestInserter(11));
        assert_eq!(&*world.fetch(right).unwrap(), &TestGoodInserter(12));
    }
}
