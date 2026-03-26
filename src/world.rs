use std::{
    any::{Any, TypeId, type_name},
    cell::{Cell, RefCell},
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
        write!(f, "#{}", self.0.0)
    }
}

impl<T: ?Sized> From<Handle<T>> for HandleInfo {
    fn from(value: Handle<T>) -> Self {
        HandleInfo(value.cast(), type_name::<T>())
    }
}

/// Represent a view of world.
pub struct ViewId(usize);

impl Clone for ViewId {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for ViewId {}

impl PartialEq for ViewId {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for ViewId {}

impl Hash for ViewId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "View({})", self.0)
    }
}

impl fmt::Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.0)
    }
}

// World Management //

// Center of multiple accesses in world, which also prevents constructional changes
pub struct World {
    elem_idx: RefCell<Handle>,
    view_idx: RefCell<ViewId>,

    location: Cell<ViewId>,

    typetable: HashMap<Handle, TypeId>,
    viewtable: HashMap<Handle, ViewId>,
    storages: HashMap<TypeId, Box<dyn StorageGeneral>>,

    occupied: RefCell<HashMap<Handle, isize>>,
    inserted: RefCell<HashSet<Handle>>,
    removed: RefCell<HashSet<Handle>>,

    queue: Receiver<WorldCommand>,
    commander: Sender<WorldCommand>,
}

struct Storage<T: Element>(HashMap<Handle, T>);

trait StorageGeneral: Any {
    fn remove(&mut self, handle: Handle);
    fn when_remove(&mut self, world: &World, handle: Handle);
}

impl<T: Element> StorageGeneral for Storage<T> {
    fn remove(&mut self, handle: Handle) {
        self.0.remove(&handle);
    }

    fn when_remove(&mut self, world: &World, handle: Handle) {
        let elem = self.0.get_mut(&handle).unwrap();
        T::when_remove(elem, world, handle.cast());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("{0:?} was just inserted")]
    JustInserted(HandleInfo),

    #[error("{0:?} was just removed")]
    JustRemoved(HandleInfo),

    #[error("{0:?} does not exist")]
    InvalidHandle(HandleInfo),

    #[error("{0:?} is in {1:?}, not here {2:?}")]
    Invisible(HandleInfo, ViewId, ViewId),

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
            elem_idx: RefCell::new(Handle(0, PhantomData)),
            view_idx: RefCell::new(ViewId(1)),
            location: Cell::new(ViewId(0)),
            typetable: HashMap::new(),
            viewtable: HashMap::new(),
            storages: HashMap::new(),
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
        let mut elem_idx = self.elem_idx.borrow_mut();
        let handle = elem_idx.cast::<T>();
        elem_idx.0 += 1;

        // write immediate record
        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(handle.cast());

        // delay execution
        let location = self.location.get();
        self.queue(move |world| {
            // get type table ready
            let storage = world.storages.entry(TypeId::of::<T>()).or_insert_with(|| {
                log::debug!("register elements: {}", type_name::<T>());
                Box::new(Storage::<T>(HashMap::new()))
            });

            // push into storage
            let storage = (storage.as_mut() as &mut dyn Any)
                .downcast_mut::<Storage<T>>()
                .unwrap();
            storage.0.insert(handle.cast(), element);

            // update typetable
            world.typetable.insert(handle.cast(), TypeId::of::<T>());
            world.viewtable.insert(handle.cast(), location);

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
        let type_id = *self.typetable.get(&handle_any).unwrap();
        let storage = self.storages.get(&type_id).unwrap().as_ref() as *const _;
        let storage = storage as *mut dyn StorageGeneral;
        unsafe { (*storage).when_remove(self, handle_any) };

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
            // update typetable
            world.typetable.remove(&handle.cast());
            world.viewtable.remove(&handle.cast());

            // pop out storage
            let storage = world.storages.get_mut(&type_id).unwrap();
            storage.remove(handle_any);

            log::trace!("remove {:?}", handle);
        });

        Ok(cnt)
    }

    // views //

    /// Create a new fresh view.
    pub fn view(&self) -> ViewId {
        let mut view_idx = self.view_idx.borrow_mut();
        let view = *view_idx;
        view_idx.0 += 1;
        view
    }

    /// Enter view.
    pub fn enter(&self, view: ViewId, f: impl FnOnce()) {
        let origin = self.location.get();
        self.location.set(view);
        f();
        self.location.set(origin);
    }

    // commands //

    pub fn commander(&self) -> Commander {
        Commander {
            location: self.location.get(),
            inner: self.commander.clone(),
        }
    }

    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let result = self.commander.send(WorldCommand {
            location: self.location.get(),
            action: Box::new(f),
        });
        if let Err(err) = result {
            log::error!("error in world queue ops: {err}");
        }
    }

    pub fn flush(&mut self) {
        let origin = self.location.get();
        let buf = self.queue.try_iter().collect::<Vec<_>>();
        for cmd in buf {
            self.location.set(cmd.location);
            (cmd.action)(self);
            self.flush();
        }
        self.location.set(origin);
    }

    // validation //

    /// Check whether target element exists, insertion without `flush` will *NOT* be included.
    pub fn validate<T: ?Sized>(&self, handle: Handle<T>) -> Result<(), WorldError> {
        if self.removed.borrow().contains(&handle.cast()) {
            return Err(WorldError::JustRemoved(handle.into()));
        }

        if !self.typetable.contains_key(&handle.cast()) {
            if self.inserted.borrow().contains(&handle.cast()) {
                return Err(WorldError::JustInserted(handle.into()));
            }

            return Err(WorldError::InvalidHandle(handle.into()));
        }

        if let Some(&target) = self.viewtable.get(&handle.cast()) {
            let here = self.location.get();
            if target != here {
                return Err(WorldError::Invisible(handle.into(), target, here));
            }
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

    // fetch //

    pub fn fetch<T: Element>(&self, handle: Handle<T>) -> Result<Ref<'_, T>, WorldError> {
        self.available(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() += 1;

        let storage = (self.storages)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::UnmatchedType(handle.into()))?;
        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();
        let element = (storage.0)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(handle.into()))? as *const _;

        Ok(Ref {
            ptr: element,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: Handle<T>) -> Result<RefMut<'_, T>, WorldError> {
        self.available_mut(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() -= 1;

        let storage = (self.storages)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::UnmatchedType(handle.into()))?;
        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();
        let element = (storage.0)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(handle.into()))? as *const _;
        let element = element as *mut T;

        Ok(RefMut {
            ptr: element,
            world: self,
            handle,
            modified: false,
        })
    }

    // singleton //

    pub fn single<T: Element>(&self) -> Result<Handle<T>, WorldError> {
        let storage = (self.storages)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::SingletonNoSuch(type_name::<T>()))?;
        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();

        let mut ret = None;
        let mut cnt = 0;
        for &handle in storage.0.keys() {
            if self.validate(handle).is_err() {
                continue;
            }

            cnt += 1;
            ret.replace(handle);
        }

        let Some(ret) = ret else {
            return Err(WorldError::SingletonNoSuch(type_name::<T>()));
        };

        if cnt > 1 {
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

    /// The actual number of element would be equal or less than this number.
    pub fn size_hint<T: Element>(&self) -> usize {
        (self.storages)
            .get(&TypeId::of::<T>())
            .map(|storage| {
                let storage = (storage.as_ref() as &dyn Any)
                    .downcast_ref::<Storage<T>>()
                    .unwrap();
                storage.0.len()
            })
            .unwrap_or_default()
    }

    pub fn foreach<T: Element>(&self, mut f: impl FnMut(Handle<T>)) {
        let Some(storage) = self.storages.get(&TypeId::of::<T>()) else {
            return;
        };

        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();

        for &handle in storage.0.keys() {
            if self.validate(handle).is_err() {
                continue;
            }

            f(handle.cast());
        }
    }

    pub fn foreach_fetch<T: Element>(&self, mut f: impl FnMut(Ref<T>)) {
        self.foreach::<T>(|handle| f(self.fetch(handle).unwrap()))
    }

    pub fn foreach_fetch_mut<T: Element>(&self, mut f: impl FnMut(RefMut<T>)) {
        self.foreach::<T>(|handle| f(self.fetch_mut(handle).unwrap()))
    }

    // observer & trigger //

    pub fn observer<T: ?Sized + 'static, E: 'static>(
        &self,
        target: Handle<T>,
        mut action: impl FnMut(&E, &World) + 'static,
    ) -> Handle {
        let handle = self.insert(Observer {
            action: Box::new(move |event, world| action(event, world)),
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
            || (!self.typetable.contains_key(&depend_on.cast())
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

struct WorldCommand {
    location: ViewId,
    action: Box<dyn FnOnce(&mut World)>,
}

/// A flexible command access to world.
#[derive(Debug, Clone)]
pub struct Commander {
    location: ViewId,
    inner: Sender<WorldCommand>,
}

impl Commander {
    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let result = self.inner.send(WorldCommand {
            location: self.location,
            action: Box::new(f),
        });

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

        world.observer(left, move |TestEvent(i), world| {
            let mut this = world.fetch_mut(left).unwrap();
            this.0 += i;
        });

        let obs = world.observer(left, move |TestEvent(i), world| {
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
