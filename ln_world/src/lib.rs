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
use indexmap::IndexMap;
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

// SAFETY: Introduce by PhantomData, which is totally safe for handle
unsafe impl<T: ?Sized> Send for Handle<T> {}

// SAFETY: Introduce by PhantomData, which is totally safe for handle
unsafe impl<T: ?Sized> Sync for Handle<T> {}

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
    pub const fn untyped(self) -> Handle<dyn Any> {
        self.cast()
    }
}

impl<T: ?Sized> Handle<T> {
    const fn cast<U: ?Sized>(self) -> Handle<U> {
        Handle(self.0, PhantomData)
    }
}

/// Handle with debug information.
#[derive(Clone, Copy)]
pub struct HandleInfo(Handle, &'static str);

impl fmt::Debug for HandleInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", self.1, self.0.0)
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

// World Management //

// Center of multiple accesses in world, which also prevents constructional changes
pub struct World {
    elem_idx: RefCell<Handle>,

    indices: HashMap<Handle, HandleIndex>,
    storages: HashMap<TypeId, Box<dyn StorageGeneral>>,
    cache: RefCell<HashMap<(TypeId, Handle), SmallVec<[usize; 1]>>>,

    occupied: RefCell<HashMap<Handle, isize>>,
    inserted: RefCell<HashSet<Handle>>,
    removed: RefCell<HashSet<Handle>>,

    location: Cell<Handle>,
    dependencies: RefCell<Dependencies>,

    queue: Receiver<WorldCommand>,
    commander: Sender<WorldCommand>,
}

struct HandleIndex {
    tid: TypeId,
    view: Handle,
    elemrefs: Vec<Handle>,
    viewrefs: Vec<Handle>,
}

struct Storage<T: Element>(IndexMap<Handle, T>);

trait StorageGeneral: Any {
    fn name(&self) -> &'static str;
    fn remove(&mut self, handle: Handle);
    fn when_remove(&mut self, world: &World, handle: Handle);
}

impl<T: Element> StorageGeneral for Storage<T> {
    fn name(&self) -> &'static str {
        type_name::<T>()
    }

    fn remove(&mut self, handle: Handle) {
        self.0.swap_remove(&handle);
    }

    fn when_remove(&mut self, world: &World, handle: Handle) {
        let elem = self.0.get_mut(&handle).unwrap();
        T::when_remove(elem, world, handle.cast());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("{0:?} was just inserted, not flushed yet")]
    JustInserted(HandleInfo),

    #[error("{0:?} was just removed, not flushed yet")]
    JustRemoved(HandleInfo),

    #[error("{0:?} was removed")]
    Removed(HandleInfo),

    #[error("{0:?} does not exist")]
    InvalidHandle(HandleInfo),

    #[error("{0:?} is invisible in {1:?} from here {2:?}")]
    Invisible(HandleInfo, HandleInfo, HandleInfo),

    #[error("{0:?} serves as root view element")]
    Initelem(HandleInfo),

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

    #[error("{0} may be singleton, but not flushed")]
    SingletonCorrupted(&'static str),
}

impl World {
    pub fn new() -> Self {
        let (commander, queue) = channel();

        let initelem_index = HandleIndex {
            tid: TypeId::of::<()>(),
            view: INITELEM,
            elemrefs: Vec::new(),
            viewrefs: Vec::new(),
        };

        let mut indices = HashMap::new();
        indices.insert(INITELEM, initelem_index);

        World {
            elem_idx: RefCell::new(Handle(1, PhantomData)),
            indices,
            storages: HashMap::new(),
            cache: RefCell::default(),
            occupied: RefCell::default(),
            inserted: RefCell::default(),
            removed: RefCell::default(),
            location: Cell::new(INITELEM),
            dependencies: RefCell::default(),
            queue,
            commander,
        }
    }

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
                log::trace!("register elements: {}", type_name::<T>());
                Box::new(Storage::<T>(IndexMap::new()))
            });

            // push into storage
            let storage = (storage.as_mut() as &mut dyn Any)
                .downcast_mut::<Storage<T>>()
                .unwrap();
            storage.0.insert(handle.cast(), element);

            // update typetable
            world.indices.insert(
                handle.cast(),
                HandleIndex {
                    tid: TypeId::of::<T>(),
                    view: location,
                    elemrefs: Vec::new(),
                    viewrefs: Vec::new(),
                },
            );
            world.inserted.get_mut().remove(&handle.cast());

            // junk cache
            let mut cache = world.cache.borrow_mut();
            cache.retain(|(t, _), _| *t != TypeId::of::<T>());
            drop(cache);

            // when_insert
            let mut element = world.fetch_mut(handle).unwrap();
            element.when_insert(world, handle);
        });

        handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the owned value of removed element.
    pub fn remove(&self, handle: Handle<impl ?Sized + 'static>) -> Result<usize, WorldError> {
        self.available_mut(handle)?;
        let mut cnt = 1;

        // when_remove
        // SAFETY: we have checked the mutability
        let tid = self.indices.get(&handle.cast()).unwrap().tid;
        let storage = self.storages.get(&tid).unwrap().as_ref() as *const _;
        let storage = storage as *mut dyn StorageGeneral;
        unsafe { (*storage).when_remove(self, handle.cast()) };

        // clear view
        self.enter(handle, || {
            cnt += self.clear();
        });

        // cleanup parents' dependencies
        let mut dependencies = self.dependencies.borrow_mut();
        if let Some(deps) = dependencies.0.get_mut(&handle.cast()) {
            for parent in std::mem::take(&mut deps.parents) {
                let parent_deps = dependencies.0.get_mut(&parent).unwrap();
                parent_deps.children.retain(|child| *child != handle.cast());
            }
        }
        drop(dependencies);

        // remove children
        loop {
            let mut dependencies = self.dependencies.borrow_mut();
            let Some(dep) = dependencies.0.get(&handle.cast()) else {
                break;
            };

            let Some(&child) = dep.children.last() else {
                dependencies.0.remove(&handle.cast());
                break;
            };

            drop(dependencies);

            let child_view = self.indices.get(&child).unwrap().view;
            cnt += self.enter(child_view, || self.remove(child))?;

            // TODO proper ops
            // cnt += self.remove(child)?;
        }

        // write immediate record
        let mut removed = self.removed.borrow_mut();
        removed.insert(handle.cast());
        drop(removed);

        // junk cache:
        // swap_remove shifts the storage indices the cache points to, so drop every
        // cache entry of this type, not just the one at the current location
        let mut cache = self.cache.borrow_mut();
        cache.retain(|(t, _), _| *t != tid);
        drop(cache);

        self.queue(move |world| {
            // update typetable
            world.indices.remove(&handle.cast());

            // pop out storage
            let storage = world.storages.get_mut(&tid).unwrap();
            storage.remove(handle.cast());
        });

        Ok(cnt)
    }

    // views //

    pub fn here(&self) -> Handle {
        self.location.get()
    }

    /// Enter view.
    pub fn enter<R>(&self, view: Handle<impl ?Sized>, f: impl FnOnce() -> R) -> R {
        let origin = self.location.get();
        self.location.set(view.cast());
        let ret = f();
        self.location.set(origin);
        ret
    }

    pub fn enter_insert<T: Element>(&self, view: Handle<impl ?Sized>, element: T) -> Handle<T> {
        self.enter(view, || self.insert(element))
    }

    pub fn enter_single_fetch<T: Element>(
        &self,
        view: Handle<impl ?Sized>,
    ) -> Result<Ref<'_, T>, WorldError> {
        self.enter(view, || self.single_fetch())
    }

    pub fn enter_single_fetch_mut<T: Element>(
        &self,
        view: Handle<impl ?Sized>,
    ) -> Result<RefMut<'_, T>, WorldError> {
        self.enter(view, || self.single_fetch_mut())
    }

    pub fn enter_single_remove<T: Element>(
        &self,
        view: Handle<impl ?Sized>,
    ) -> Result<usize, WorldError> {
        self.enter(view, || self.single_remove::<T>())
    }

    pub fn enter_queue(&self, view: Handle<impl ?Sized>, f: impl FnOnce(&mut World) + 'static) {
        self.enter(view, || self.queue(f))
    }

    /// Clear all elements from current view.
    pub fn clear(&self) -> usize {
        let mut cnt = 0;
        for (&handle, index) in self.indices.iter() {
            if index.view != self.location.get() {
                continue;
            }

            if self.validate(handle).is_err() {
                continue;
            }

            cnt += self.remove(handle).unwrap();
        }

        cnt
    }

    // cache //

    pub fn queue_cache<T: Element>(&self) -> bool {
        let tid = TypeId::of::<T>();
        let here = self.location.get();
        self.queue(|world| {
            world.cache::<T>();
        });
        let cache = self.cache.borrow();
        !cache.contains_key(&(tid, here))
    }

    // return if a new cache is established
    pub fn cache<T: Element>(&mut self) -> bool {
        let tid = TypeId::of::<T>();
        let here = self.location.get();

        let cache = self.cache.get_mut();
        if cache.contains_key(&(tid, here)) {
            return false;
        }

        let Some(storage) = self.storages.get(&tid) else {
            return false;
        };

        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();

        let mut cached = SmallVec::new();
        for (i, &handle) in storage.0.keys().enumerate() {
            if self.validate(handle).is_err() {
                continue;
            }

            cached.push(i);
        }

        let cache = self.cache.get_mut();
        cache.insert((tid, here), cached);

        return true;
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

    fn get_storage<T: Element>(&self, tid: TypeId) -> Option<&Storage<T>> {
        let Some(storage) = self.storages.get(&tid) else {
            return None;
        };

        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();

        Some(storage)
    }

    // validation //

    /// Check whether target element exists, insertion without `flush` will *NOT* be included.
    ///
    /// Validated handles are:
    /// - Not Initelem handle
    /// - Not an element just insert
    /// - Not pointing to removed/invalid element
    /// - Visible in current view node
    ///
    /// Validated handles are possible to be:
    /// - With wrong type signature
    /// - Unsafe to Immutably/mutably borrow
    ///
    /// Functions with `_unchecked` means they skip the validation part, since view visibility
    /// evaluating costs time, which is sensitive in some cases.
    pub fn validate(&self, handle: Handle<impl ?Sized>) -> Result<(), WorldError> {
        if handle.0 == 0 {
            return Err(WorldError::Initelem(self.info(handle)));
        }

        if self.removed.borrow().contains(&handle.cast()) {
            if self.indices.contains_key(&handle.cast()) {
                return Err(WorldError::JustRemoved(self.info(handle)));
            }

            return Err(WorldError::Removed(self.info(handle)));
        }

        let Some(index) = self.indices.get(&handle.cast()) else {
            if self.inserted.borrow().contains(&handle.cast()) {
                return Err(WorldError::JustInserted(self.info(handle)));
            }

            return Err(WorldError::InvalidHandle(self.info(handle)));
        };

        let here = self.location.get();

        // 1. plain
        if index.view == here {
            return Ok(());
        }

        // 2. elemref
        if index.elemrefs.contains(&here) {
            return Ok(());
        }

        // 3. viewref
        let mut visible = vec![index.view];
        visible.append(&mut index.elemrefs.clone());
        let mut frnt = 0;
        while let Some(&view) = visible.get(frnt) {
            if view == here {
                return Ok(());
            }

            let Some(view_index) = self.indices.get(&view) else {
                frnt += 1;
                continue;
            };

            for &viewref in &view_index.viewrefs {
                if !visible.contains(&viewref) {
                    visible.push(viewref);
                }
            }

            frnt += 1;
        }

        Err(WorldError::Invisible(
            self.info(handle),
            self.info(index.view),
            self.info(here),
        ))
    }

    /// Check whether target element can be borrowed immutably, insertion without
    /// `flush` will *NOT* be included.
    pub fn available(&self, handle: Handle<impl ?Sized>) -> Result<(), WorldError> {
        self.validate(handle)?;
        self.available_unchecked(handle)
    }

    /// Check whether target element can be borrowed immutably, insertion without
    /// `flush` will *NOT* be included.
    fn available_unchecked(&self, handle: Handle<impl ?Sized>) -> Result<(), WorldError> {
        let occupied = self.occupied.borrow();
        if occupied.get(&handle.cast()).is_some_and(|cnt| *cnt < 0) {
            panic!("{}", WorldError::Unavailable(self.info(handle)));
        }

        Ok(())
    }

    /// Check whether target element can be borrowed mutably, insertion without
    /// `flush` will *NOT* be included.
    pub fn available_mut(&self, handle: Handle<impl ?Sized>) -> Result<(), WorldError> {
        self.validate(handle)?;
        self.available_mut_unchecked(handle)
    }

    /// Check whether target element can be borrowed mutably, insertion without
    /// `flush` will *NOT* be included.
    fn available_mut_unchecked(&self, handle: Handle<impl ?Sized>) -> Result<(), WorldError> {
        let occupied = self.occupied.borrow();
        if occupied.get(&handle.cast()).is_some_and(|cnt| *cnt != 0) {
            panic!("{}", WorldError::UnavailableMut(self.info(handle)));
        }

        Ok(())
    }

    // fetch //

    pub fn fetch<T: Element>(&self, handle: Handle<T>) -> Result<Ref<'_, T>, WorldError> {
        self.validate(handle)?;
        self.fetch_unchecked(handle)
    }

    fn fetch_unchecked<T: Element>(&self, handle: Handle<T>) -> Result<Ref<'_, T>, WorldError> {
        self.available_unchecked(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() += 1;

        let storage = (self.storages)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::UnmatchedType(self.info(handle)))?;
        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();
        let element = (storage.0)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(self.info(handle)))? as *const _;

        Ok(Ref {
            ptr: element,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: Handle<T>) -> Result<RefMut<'_, T>, WorldError> {
        self.validate(handle)?;
        self.fetch_mut_unchecked(handle)
    }

    fn fetch_mut_unchecked<T: Element>(
        &self,
        handle: Handle<T>,
    ) -> Result<RefMut<'_, T>, WorldError> {
        self.available_mut_unchecked(handle)?;

        let mut occupied = self.occupied.borrow_mut();
        *occupied.entry(handle.cast()).or_default() -= 1;

        let storage = (self.storages)
            .get(&TypeId::of::<T>())
            .ok_or(WorldError::UnmatchedType(self.info(handle)))?;
        let storage = (storage.as_ref() as &dyn Any)
            .downcast_ref::<Storage<T>>()
            .unwrap();
        let element = (storage.0)
            .get(&handle.cast())
            .ok_or(WorldError::InvalidHandle(self.info(handle)))? as *const _;
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
        let tid = TypeId::of::<T>();
        let here = self.location.get();

        let Some(storage) = self.get_storage::<T>(tid) else {
            return Err(WorldError::SingletonNoSuch(type_name::<T>()));
        };

        let mut ret = None;
        let mut cnt = 0;
        let mut corrupted = 0;
        let cache = self.cache.borrow();
        if let Some(cached) = cache.get(&(tid, here)) {
            for &cache in cached {
                let Some((&handle, _)) = storage.0.get_index(cache) else {
                    log::warn!("cache failed");
                    continue;
                };

                cnt += 1;
                ret.replace(handle);
            }
        } else {
            for &handle in storage.0.keys() {
                match self.validate(handle) {
                    Ok(_) => {
                        cnt += 1;
                        ret.replace(handle);
                    }
                    Err(WorldError::JustRemoved(_) | WorldError::JustInserted(_)) => {
                        corrupted += 1;
                    }
                    Err(_) => continue,
                }
            }
        }

        if corrupted != 0 {
            return Err(WorldError::SingletonCorrupted(type_name::<T>()));
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
        self.fetch_unchecked(self.single::<T>()?)
    }

    pub fn single_fetch_mut<T: Element>(&self) -> Result<RefMut<'_, T>, WorldError> {
        self.fetch_mut_unchecked(self.single::<T>()?)
    }

    pub fn single_remove<T: Element>(&self) -> Result<usize, WorldError> {
        self.remove(self.single::<T>()?)
    }

    // iteration //

    /// The actual number of element would be equal or less than this number.
    pub fn size_hint<T: Element>(&self) -> usize {
        let tid = TypeId::of::<T>();
        let here = self.location.get();

        let cache = self.cache.borrow();
        if let Some(cached) = cache.get(&(tid, here)) {
            cached.len()
        } else {
            (self.storages)
                .get(&tid)
                .map(|storage| {
                    let storage = (storage.as_ref() as &dyn Any)
                        .downcast_ref::<Storage<T>>()
                        .unwrap();
                    storage.0.len()
                })
                .unwrap_or_default()
        }
    }

    pub fn foreach<T: Element>(&self, mut f: impl FnMut(Handle<T>)) {
        let tid = TypeId::of::<T>();
        let here = self.location.get();

        let Some(storage) = self.get_storage::<T>(tid) else {
            return;
        };

        // cache hit optimization
        let cache = self.cache.borrow();
        if let Some(cached) = cache.get(&(tid, here)) {
            for &cache in cached {
                let Some((handle, _)) = storage.0.get_index(cache) else {
                    log::warn!("cache failed");
                    continue;
                };

                f(handle.cast());
            }
        } else {
            for &handle in storage.0.keys() {
                if self.validate(handle).is_err() {
                    continue;
                }

                f(handle.cast());
            }
        }
    }

    pub fn foreach_enter<T: Element>(&self, mut f: impl FnMut(Handle<T>)) {
        self.foreach::<T>(|handle| self.enter(handle, || f(handle)));
    }

    pub fn foreach_fetch<T: Element>(&self, mut f: impl FnMut(Ref<T>)) {
        self.foreach::<T>(|handle| f(self.fetch_unchecked(handle).unwrap()))
    }

    pub fn foreach_fetch_mut<T: Element>(&self, mut f: impl FnMut(RefMut<T>)) {
        self.foreach::<T>(|handle| f(self.fetch_mut_unchecked(handle).unwrap()))
    }

    // observer & trigger //

    pub fn observer<E: 'static>(
        &self,
        target: Handle<impl ?Sized + 'static>,
        action: impl FnMut(&E, &World) + 'static,
    ) -> Handle {
        let here = self.location.get();
        let handle = self.enter(INITELEM, || {
            self.insert(Observer {
                action: Box::new(action),
                view: here,
                target: target.cast(),
            })
        });

        handle.cast()
    }

    /// Will immediately triggered and acquire mutable access to `target`.
    pub fn trigger<E: 'static>(&self, target: Handle<impl ?Sized + 'static>, event: &E) -> usize {
        // if let Err(e) = self.validate(target) {
        //     log::error!("trigger on invalidated target: {e:?}");
        // }

        let mut cnt = 0;
        self.enter(INITELEM, || {
            if let Ok(observers) = self.single_fetch::<Observers<E>>()
                && let Some(observers) = observers.members.get(&target.cast())
            {
                for mut observer in observers.iter().filter_map(|x| self.fetch_mut(*x).ok()) {
                    self.enter(observer.view, || (observer.action)(event, self));
                    cnt += 1;
                }
            }
        });

        cnt
    }

    pub fn queue_trigger<E: 'static>(&self, target: Handle<impl ?Sized + 'static>, event: E) {
        self.queue(move |world| {
            world.trigger(target, &event);
        });
    }

    // dependency //

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn dependency(&self, child: Handle<impl ?Sized>, parent: Handle<impl ?Sized>) {
        if let Err(e) = self.validate(parent)
            && !matches!(e, WorldError::JustInserted(_) | WorldError::Invisible(..))
        {
            let err = WorldError::ToxicDependency(self.info(child), self.info(parent));
            log::error!("failed to attach dependency: {err:?}");
            return;
        }

        let child = child.cast();
        let parent = parent.cast();

        let mut dependencies = self.dependencies.borrow_mut();
        let parent_deps = dependencies.0.entry(parent).or_default();
        parent_deps.children.push(child);
        let child_deps = dependencies.0.entry(child).or_default();
        child_deps.parents.push(parent);
    }

    fn info<T: ?Sized>(&self, value: Handle<T>) -> HandleInfo {
        HandleInfo(
            value.cast(),
            self.indices
                .get(&value.cast())
                .and_then(|x| self.storages.get(&x.tid))
                .map(|x| x.name())
                .unwrap_or("invalid"),
        )
    }
}

impl Default for World {
    fn default() -> Self {
        World::new()
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
        let cnt = occupied.get_mut(&self.handle.cast()).unwrap();
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
        let cnt = occupied.get_mut(&self.handle.cast()).unwrap();
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

// View //

const INITELEM: Handle = Handle(0, PhantomData);

/// refer another element in other views
pub struct ElemRef(pub Handle);

// refer all elements from other views
pub struct ViewRef(pub Handle);

impl Element for ElemRef {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let target = self.0;
        let here = world.location.get();
        world.queue(move |world| {
            let mut cache = world.cache.borrow_mut();
            cache.retain(|(_, view), _| view != &here);

            let Some(index) = world.indices.get_mut(&target) else {
                log::error!("Fatal error: ElemRef cannot find content handle");
                return;
            };

            index.elemrefs.push(here);

            world.dependency(this, target);
        });
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        let target = self.0;
        let here = world.location.get();
        world.queue(move |world| {
            let mut cache = world.cache.borrow_mut();
            cache.retain(|(_, view), _| view != &here);

            let Some(index) = world.indices.get_mut(&target) else {
                // silent skip removed node
                return;
            };

            let mut t = None;
            for (i, &p) in index.elemrefs.iter().enumerate() {
                if p == here {
                    t.replace(i);
                }
            }
            if let Some(i) = t {
                index.elemrefs.swap_remove(i);
            } else {
                log::error!("Fail to retrieve elemref")
            }
        });
    }
}

impl Element for ViewRef {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let target = self.0;
        let here = world.location.get();
        world.queue(move |world| {
            let mut cache = world.cache.borrow_mut();
            cache.retain(|(_, view), _| view != &here);

            let Some(index) = world.indices.get_mut(&target) else {
                log::error!("Fatal error: ElemRef cannot find content handle");
                return;
            };

            index.viewrefs.push(here);

            world.dependency(this, target);
        });
    }

    fn when_remove(&mut self, world: &World, _this: Handle<Self>) {
        let target = self.0;
        let here = world.location.get();
        world.queue(move |world| {
            let mut cache = world.cache.borrow_mut();
            cache.retain(|(_, view), _| view != &here);

            let Some(index) = world.indices.get_mut(&target) else {
                // silent skip removed node
                return;
            };

            let mut t = None;
            for (i, &p) in index.viewrefs.iter().enumerate() {
                if p == here {
                    t.replace(i);
                }
            }
            if let Some(i) = t {
                index.viewrefs.swap_remove(i);
            } else {
                log::error!("Fail to retrieve viewref")
            }
        });
    }
}

impl Element for () {}

// Commander //

struct WorldCommand {
    location: Handle,
    action: Box<dyn FnOnce(&mut World)>,
}

/// A flexible command access to world.
#[derive(Debug, Clone)]
pub struct Commander {
    location: Handle,
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
    view: Handle,
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

                log::trace!("register events: {}", type_name::<E>());

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

#[derive(Default, Clone)]
struct Dependency {
    parents: SmallVec<[Handle; 1]>,
    children: SmallVec<[Handle; 4]>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestBlanker;
    impl Element for TestBlanker {}

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
    fn multi_dependency_children() {
        let mut world = World::default();

        let parent = world.insert(TestInserter(0));
        let child1 = world.insert(TestInserter(1));
        let child2 = world.insert(TestInserter(1));
        let child3 = world.insert(TestInserter(1));

        world.flush();

        world.dependency(child1, parent);
        world.dependency(child2, parent);
        world.dependency(child3, parent);

        world.remove(parent).unwrap();

        world.flush();

        assert!(world.validate(parent).is_err());
        assert!(world.validate(child1).is_err());
        assert!(world.validate(child2).is_err());
        assert!(world.validate(child3).is_err());
    }

    #[test]
    fn multi_dependency_parent() {
        let mut world = World::default();

        let child = world.insert(TestInserter(0));
        let child3 = world.insert(TestInserter(0));
        let parent1 = world.insert(TestInserter(1));
        let parent2 = world.insert(TestInserter(1));
        let parent3 = world.insert(TestInserter(1));

        world.flush();

        world.dependency(child, parent1);
        world.dependency(child, parent2);
        world.dependency(child, parent3);
        world.dependency(child3, parent3);

        world.remove(parent1).unwrap();
        world.remove(parent3).unwrap();

        world.flush();

        assert!(world.validate(child).is_err());
        assert!(world.validate(child3).is_err());
        assert!(world.validate(parent1).is_err());
        assert!(world.validate(parent2).is_ok());
        assert!(world.validate(parent3).is_err());
    }

    #[test]
    fn multi_dependency_grand_parent() {
        let mut world = World::default();

        let grand_parent = world.insert(TestInserter(0));
        let parent = world.insert(TestInserter(1));
        let child = world.insert(TestInserter(2));

        world.flush();

        world.dependency(parent, grand_parent);
        world.dependency(child, parent);
        world.dependency(child, grand_parent);

        world.remove(grand_parent).unwrap();

        world.flush();

        assert!(world.validate(grand_parent).is_err());
        assert!(world.validate(parent).is_err());
        assert!(world.validate(child).is_err());
    }

    #[test]
    fn views() {
        let mut world = World::default();

        let view1 = world.insert(TestBlanker);
        let view2 = world.insert(TestBlanker);

        let node1 = world.enter(view1, || world.insert(TestInserter(1)));
        let node2 = world.enter(view2, || world.insert(TestInserter(2)));

        world.flush();

        assert!(world.enter(view1, || world.validate(node1).is_ok()));
        assert!(world.enter(view2, || world.validate(node2).is_ok()));
        assert!(world.enter(view1, || world.validate(node2).is_err()));
        assert!(world.enter(view2, || world.validate(node1).is_err()));
    }

    #[test]
    fn view_refs_deps() {
        let mut world = World::default();

        let view1 = world.insert(TestBlanker);
        let view2 = world.insert(TestBlanker);
        let view3 = world.insert(TestBlanker);

        let node1 = world.enter(view1, || world.insert(TestInserter(1)));
        let node2 = world.enter(view2, || world.insert(TestInserter(2)));
        let node3 = world.enter(view3, || world.insert(TestInserter(3)));

        world.enter(view3, || world.dependency(node3, node1));

        world.enter(view2, || world.insert(ViewRef(view1.untyped())));
        world.enter(view3, || world.insert(ViewRef(view2.untyped())));

        world.flush();

        world.enter(view1, || world.remove(node1).unwrap());

        world.flush();

        assert!(world.enter(view3, || world.validate(node1).is_err()));
        assert!(world.enter(view3, || world.validate(node2).is_ok()));
        assert!(world.enter(view3, || world.validate(node3).is_err()));
    }

    #[test]
    fn view_refs_chain() {
        let mut world = World::default();

        let view1 = world.insert(TestBlanker);
        let view2 = world.insert(TestBlanker);
        let view3 = world.insert(TestBlanker);

        let node1 = world.enter(view1, || world.insert(TestInserter(1)));
        let node2 = world.enter(view2, || world.insert(TestInserter(2)));
        let node3 = world.enter(view3, || world.insert(TestInserter(3)));

        world.enter(view2, || world.insert(ViewRef(view1.untyped())));
        world.enter(view2, || world.insert(ViewRef(view3.untyped())));
        world.enter(view3, || world.insert(ViewRef(view2.untyped())));

        world.flush();

        assert!(world.enter(view3, || world.validate(node1).is_ok()));
        assert!(world.enter(view2, || world.validate(node1).is_ok()));
        assert!(world.enter(view1, || world.validate(node1).is_ok()));

        assert!(world.enter(view3, || world.validate(node2).is_ok()));
        assert!(world.enter(view2, || world.validate(node2).is_ok()));
        assert!(world.enter(view1, || world.validate(node2).is_err()));

        assert!(world.enter(view3, || world.validate(node3).is_ok()));
        assert!(world.enter(view2, || world.validate(node3).is_ok()));
        assert!(world.enter(view1, || world.validate(node3).is_err()));
    }

    #[test]
    fn elem_refs_chain() {
        let mut world = World::default();

        let view1 = world.insert(TestBlanker);
        let view2 = world.insert(TestBlanker);
        let view3 = world.insert(TestBlanker);

        let node1a = world.enter(view1, || world.insert(TestInserter(11)));
        let node1b = world.enter(view1, || world.insert(TestInserter(12)));
        let node2 = world.enter(view2, || world.insert(TestInserter(2)));
        let node3 = world.enter(view3, || world.insert(TestInserter(3)));

        world.enter(view1, || world.insert(ViewRef(view2.untyped())));
        world.enter(view2, || world.insert(ElemRef(node1a.untyped())));
        world.enter(view2, || world.insert(ViewRef(view3.untyped())));
        world.enter(view3, || world.insert(ElemRef(node1b.untyped())));

        world.flush();

        assert!(world.enter(view1, || world.validate(node1a).is_ok()));
        assert!(world.enter(view1, || world.validate(node1b).is_ok()));
        assert!(world.enter(view1, || world.validate(node2).is_ok()));
        assert!(world.enter(view1, || world.validate(node3).is_ok()));

        assert!(world.enter(view2, || world.validate(node1a).is_ok()));
        assert!(world.enter(view2, || world.validate(node1b).is_ok()));
        assert!(world.enter(view2, || world.validate(node2).is_ok()));
        assert!(world.enter(view2, || world.validate(node3).is_ok()));

        assert!(world.enter(view3, || world.validate(node1a).is_err()));
        assert!(world.enter(view3, || world.validate(node1b).is_ok()));
        assert!(world.enter(view3, || world.validate(node2).is_err()));
        assert!(world.enter(view3, || world.validate(node3).is_ok()));
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

        world.observer(left, move |TestEvent(i), world| {
            let mut this = world.fetch_mut(right).unwrap();
            this.0 += i;
        });

        world.flush();

        world.trigger(left, &TestEvent(10));

        assert_eq!(&*world.fetch(left).unwrap(), &TestInserter(11));
        assert_eq!(&*world.fetch(right).unwrap(), &TestGoodInserter(12));
    }
}
