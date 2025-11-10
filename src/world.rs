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

// Element Section //

/// A shared form of objects in the [`World`].
pub trait Element: Any {}

impl dyn Element {
    pub fn is<T: Any>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}

/// Indicated that it is used for insertion
#[expect(unused_variables)]
pub trait InsertElement: Element {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {}
}

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
pub struct ElementHandle<T: ?Sized = dyn Element>(usize, PhantomData<T>);

impl<T: ?Sized> Clone for ElementHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for ElementHandle<T> {}

impl<T: ?Sized> PartialEq for ElementHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T: ?Sized> Eq for ElementHandle<T> {}

impl<T: ?Sized> Hash for ElementHandle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: ?Sized> fmt::Debug for ElementHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle<{}>({})", type_name::<T>(), self.0)
    }
}

impl<T: ?Sized> fmt::Display for ElementHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl<T: Element> From<ElementHandle<T>> for ElementHandle {
    fn from(value: ElementHandle<T>) -> Self {
        ElementHandle(value.0, PhantomData)
    }
}

impl<T: ?Sized> ElementHandle<T> {
    pub fn untyped(self) -> ElementHandle<dyn Element> {
        self.cast()
    }

    fn cast<U: ?Sized>(self) -> ElementHandle<U> {
        ElementHandle(self.0, PhantomData)
    }
}

// World Management //

pub struct World {
    curr_idx: ElementHandle,
    elements: HashMap<ElementHandle, Box<dyn Element>>,
    cache: HashMap<TypeId, HashSet<ElementHandle>>,
}

// Center of multiple accesses in world, which also prevents constructional changes
pub struct WorldCell<'world> {
    world: &'world mut World,
    occupied: RefCell<HashMap<ElementHandle, isize>>,
    cell_idx: RefCell<ElementHandle>,
    inserted: RefCell<HashSet<ElementHandle>>,
    removed: RefCell<HashSet<ElementHandle>>,
}

/// A full mutable world reference with specific element selected.
pub struct WorldEntry<'world, T: ?Sized> {
    world: &'world mut World,
    handle: ElementHandle<T>,
}

/// A world cell reference with specific element selected. No borrowing effect.
pub struct WorldCellEntry<'world, T: ?Sized> {
    world: &'world WorldCell<'world>,
    handle: ElementHandle<T>,
}

/// A full mutable world reference with specific element selected.
pub struct WorldOther<'world, T: ?Sized, U: ?Sized> {
    world: &'world mut World,
    handle: ElementHandle<T>,
    other: ElementHandle<U>,
}

/// A world cell reference with specific element selected. No borrowing effect.
pub struct WorldCellOther<'world, T: ?Sized, U: ?Sized> {
    world: &'world WorldCell<'world>,
    handle: ElementHandle<T>,
    other: ElementHandle<U>,
}

impl Default for World {
    fn default() -> Self {
        World {
            curr_idx: ElementHandle(0, PhantomData),
            elements: HashMap::new(),
            cache: HashMap::new(),
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // TODO destroy event for everyone
    }
}
impl Drop for WorldCell<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

impl<T: ?Sized> Clone for WorldCellEntry<'_, T> {
    fn clone(&self) -> Self {
        WorldCellEntry {
            world: self.world,
            handle: self.handle,
        }
    }
}

impl<T: ?Sized, U: ?Sized> Clone for WorldCellOther<'_, T, U> {
    fn clone(&self) -> Self {
        WorldCellOther {
            world: self.world,
            handle: self.handle,
            other: self.other,
        }
    }
}

impl World {
    pub fn insert<T: InsertElement>(&mut self, element: T) -> ElementHandle<T> {
        self.elements.insert(self.curr_idx, Box::new(element));
        let handle = self.curr_idx.cast::<T>();
        self.curr_idx.0 += 1;
        log::trace!("insert {}: {:?}", type_name::<T>(), handle);

        // update cache
        let cache = self.cache.entry(TypeId::of::<T>()).or_default();
        cache.insert(handle.untyped());

        // when_inserted
        let cell = self.cell();
        let mut element = cell.fetch_mut(handle).unwrap();
        element.when_inserted(cell.entry(handle).unwrap());
        drop(element);
        drop(cell);

        handle
    }

    pub fn remove(&mut self, handle: ElementHandle) -> Option<Box<dyn Element>> {
        let type_id = (**self.elements.get(&handle)?).type_id();
        log::trace!("remove {:?}", handle);

        // maintain dependency
        if let Some(dependencies) = self.single_fetch_mut::<Dependencies>()
            && let Some(this) = dependencies.0.remove(&handle)
        {
            // clean for parents
            for parent in this.depend_on {
                if let Some(parent) = dependencies.0.get_mut(&parent) {
                    // search for itself and swap remove
                    for i in 0..parent.depend_by.len() {
                        if parent.depend_by[i] == handle {
                            parent.depend_by.swap_remove(i);
                            break;
                        }
                    }
                }
            }

            // remove children
            for child in this.depend_by {
                self.remove(child);
            }
        }

        // trigger events
        self.entry(handle).unwrap().trigger(&Destroy);

        // update cache
        let cache = self.cache.entry(type_id).or_default();
        cache.remove(&handle);

        // TODO RemovalCapture(Box<dyn Element>)
        self.elements.remove(&handle)
    }

    pub fn contains(&self, handle: ElementHandle) -> bool {
        self.elements.contains_key(&handle)
    }

    pub fn fetch<T: Element>(&self, handle: ElementHandle<T>) -> Option<&T> {
        self.elements
            .get(&handle.untyped())
            .and_then(|element| element.downcast_ref())
    }

    pub fn fetch_mut<T: Element>(&mut self, handle: ElementHandle<T>) -> Option<&mut T> {
        self.elements
            .get_mut(&handle.untyped())
            .and_then(|element| element.downcast_mut())
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<ElementHandle<T>> {
        let mut iter = self.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(ret.cast())
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_fetch<T: Element>(&self) -> Option<&T> {
        self.fetch(self.single::<T>()?)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_fetch_mut<T: Element>(&mut self) -> Option<&mut T> {
        self.fetch_mut(self.single::<T>()?)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_entry<T: Element>(&mut self) -> Option<WorldEntry<'_, T>> {
        self.entry(self.single::<T>()?)
    }

    pub fn get<P: 'static>(&self, handle: ElementHandle) -> Option<P> {
        let getter = self.single_fetch::<PropertyGetter<P>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle)?.as_ref();
        Some(getter(element))
    }

    pub fn set<P: 'static>(&mut self, handle: ElementHandle, value: P) -> Option<()> {
        let setter = self.single_fetch::<PropertySetter<P>>()?.0.get(&handle)? as *const Box<_>;
        let element = self.elements.get_mut(&handle)?.as_mut();

        unsafe { (*setter)(element, value) };

        let getter = self.single_fetch::<PropertyGetter<P>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle).unwrap().as_ref();

        let value = getter(element);
        self.entry(handle)?.trigger(&PropertyChanged(value));

        Some(())
    }

    pub fn get_foreach<P: 'static>(&self, mut action: impl FnMut(P)) {
        if let Some(property) = self.single_fetch::<PropertyGetter<P>>() {
            for (&handle, getter) in &property.0 {
                if let Some(element) = self.elements.get(&handle) {
                    action(getter(element.as_ref()));
                }
            }
        }
    }

    pub fn set_foreach<P: 'static>(&mut self, mut action: impl FnMut() -> P) {
        if let Some(property) = self.single_fetch::<PropertySetter<P>>() {
            let property = property as *const PropertySetter<P>;
            for (&handle, setter) in unsafe { &(*property).0 } {
                if let Some(element) = self.elements.get_mut(&handle) {
                    setter(element.as_mut(), action());
                }
            }
        }
    }

    pub fn cell(&mut self) -> WorldCell<'_> {
        if self.single_fetch::<Queue>().is_none() {
            self.insert(Queue::default());
        }

        let cell_idx = self.curr_idx;
        WorldCell {
            world: self,
            occupied: RefCell::new(HashMap::new()),
            cell_idx: RefCell::new(cell_idx),
            inserted: RefCell::default(),
            removed: RefCell::default(),
        }
    }

    pub fn entry<T: ?Sized>(&mut self, handle: ElementHandle<T>) -> Option<WorldEntry<'_, T>> {
        if !self.elements.contains_key(&handle.untyped()) {
            return None;
        }

        Some(WorldEntry {
            world: self,
            handle,
        })
    }
}
impl WorldCell<'_> {
    /// Cell-mode insertion cannot perform the operation immediately so the inserted element cannot be
    /// fetched until end of the cell span. One exception is entry, which can still be used normally.
    pub fn insert<T: InsertElement>(&self, element: T) -> ElementHandle<T> {
        // get estimate_handle
        // cell-mode insertion depends on *retained* handle
        let mut cell_idx = self.cell_idx.borrow_mut();
        let estimate_handle = cell_idx.cast::<T>();
        cell_idx.0 += 1;
        log::trace!("insert {}: {:?}", type_name::<T>(), estimate_handle);

        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(estimate_handle.untyped());

        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            world
                .elements
                .insert(estimate_handle.untyped(), Box::new(element));

            // update cache
            let cache = world.cache.entry(TypeId::of::<T>()).or_default();
            cache.insert(estimate_handle.untyped());

            // when_inserted
            let cell = world.cell();
            let mut element = cell.fetch_mut(estimate_handle).unwrap();
            element.when_inserted(cell.entry(estimate_handle).unwrap());
            drop(element);
            drop(cell);
        }));

        estimate_handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the value of removed element.
    /// Notice that this removal actually ignore the borrow check so you can still preserve the reference if you have
    /// fetched it before invoking remove.
    pub fn remove(&self, handle: ElementHandle) -> usize {
        if !self.contains(handle) {
            return 0;
        }

        let type_id = (**self.world.elements.get(&handle).unwrap()).type_id();
        log::trace!("remove {:?}", handle);

        let mut cnt = 1;

        // maintain dependency
        if let Some(mut dependencies) = self.single_fetch_mut::<Dependencies>()
            && let Some(this) = dependencies.0.remove(&handle)
        {
            // clean for parents
            for parent in this.depend_on {
                if let Some(parent) = dependencies.0.get_mut(&parent) {
                    // search for itself and swap remove
                    for i in 0..parent.depend_by.len() {
                        if parent.depend_by[i] == handle {
                            parent.depend_by.swap_remove(i);
                            break;
                        }
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
        {
            let mut removed = self.removed.borrow_mut();
            removed.insert(handle);
        }

        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            // trigger events
            world.entry(handle).unwrap().trigger(&Destroy);

            // update cache
            let cache = world.cache.entry(type_id).or_default();
            cache.remove(&handle);

            // TODO RemovalCapture(Box<dyn Element>)
            world.elements.remove(&handle);
        }));

        cnt
    }

    /// Check whether target element can be borrowed immutably
    pub fn occupied(&self, handle: ElementHandle) -> bool {
        let occupied = self.occupied.borrow();
        occupied.get(&handle).is_some_and(|cnt| *cnt < 0)
    }

    /// Check whether target element can be borrowed mutably
    pub fn occupied_mut(&self, handle: ElementHandle) -> bool {
        let occupied = self.occupied.borrow();
        occupied.get(&handle).is_some_and(|cnt| *cnt != 0)
    }

    pub fn queue(&self, f: impl FnOnce(&mut World) + 'static) {
        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(f));
    }

    pub fn flush(&mut self) {
        self.world.curr_idx = *self.cell_idx.get_mut();

        let queue = self.world.single_fetch_mut::<Queue>().unwrap();
        let mut buf = Vec::new();
        buf.append(&mut queue.0);

        for cmd in buf {
            cmd(self.world);
        }
    }

    /// Insertion happened within the cell scope will not be included
    pub fn contains(&self, handle: ElementHandle) -> bool {
        if self.removed.borrow().contains(&handle) {
            return false;
        }
        self.world.contains(handle)
    }

    pub fn fetch<T: Element>(&self, handle: ElementHandle<T>) -> Option<Ref<'_, T>> {
        if self.removed.borrow().contains(&handle.untyped()) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle.untyped()).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle.untyped())?.downcast_ref()?;

        Some(Ref {
            ptr: element as *const T,
            world: self,
            handle,
        })
    }

    pub fn fetch_dyn(&self, handle: ElementHandle) -> Option<Ref<'_, dyn Element>> {
        if self.removed.borrow().contains(&handle.untyped()) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle.untyped()).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle.untyped())?.as_ref();

        Some(Ref {
            ptr: element as *const dyn Element,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: ElementHandle<T>) -> Option<RefMut<'_, T>> {
        if self.removed.borrow().contains(&handle.untyped()) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle.untyped()).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle.untyped())?.downcast_ref()?;

        Some(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut_dyn(&self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
        if self.removed.borrow().contains(&handle.untyped()) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle.untyped()).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle.untyped())?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
        })
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<ElementHandle<T>> {
        let removed = self.removed.borrow();
        let cache = self.world.cache.get(&TypeId::of::<T>())?;
        let mut iter = cache.iter().filter(|&x| !removed.contains(x));
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(ret.cast())
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_fetch<T: Element>(&self) -> Option<Ref<'_, T>> {
        self.fetch(self.single::<T>()?)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_fetch_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        self.fetch_mut(self.single::<T>()?)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_entry<T: Element>(&self) -> Option<WorldCellEntry<'_, T>> {
        self.entry(self.single::<T>()?)
    }

    pub fn get<P: 'static>(&self, handle: ElementHandle) -> Option<P> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        let property_getter = self.world.single_fetch::<PropertyGetter<P>>()?;
        let getter = property_getter.0.get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();
        Some(getter(element))
    }

    pub fn set<P: 'static>(&self, handle: ElementHandle, value: P) -> Option<()> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        let property_setter = self.world.single_fetch::<PropertySetter<P>>()?;
        let setter = property_setter.0.get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();

        let element_ptr = element as *const dyn Element as *mut dyn Element;
        setter(unsafe { element_ptr.as_mut().unwrap() }, value);

        let property_getter = self.world.single_fetch::<PropertyGetter<P>>()?;
        let getter = property_getter.0.get(&handle)?;
        let element = self.world.elements.get(&handle).unwrap().as_ref();

        drop(occupied);

        self.entry(handle)?
            .trigger(PropertyChanged(getter(element)));

        Some(())
    }

    pub fn get_foreach<P: 'static>(&self, mut action: impl FnMut(ElementHandle, P)) {
        if let Some(property) = self.world.single_fetch::<PropertyGetter<P>>() {
            let mut occupied = self.occupied.borrow_mut();
            for (&handle, getter) in &property.0 {
                let cnt = occupied.entry(handle).or_default();
                if *cnt < 0 {
                    log::error!("{handle:?} is mutably borrowed during foreach");
                    continue;
                }

                if let Some(element) = self.world.elements.get(&handle) {
                    action(handle, getter(element.as_ref()));
                }
            }
        }
    }

    pub fn set_foreach<P: 'static>(&mut self, mut action: impl FnMut(ElementHandle) -> P) {
        if let Some(property) = self.world.single_fetch::<PropertySetter<P>>() {
            let mut occupied = self.occupied.borrow_mut();
            for (&handle, setter) in &property.0 {
                let cnt = occupied.entry(handle).or_default();
                if *cnt != 0 {
                    log::error!("{handle:?} is borrowed during foreach");
                    continue;
                }

                if let Some(element) = self.world.elements.get(&handle) {
                    let element_ptr = element.as_ref() as *const dyn Element as *mut dyn Element;
                    setter(unsafe { element_ptr.as_mut().unwrap() }, action(handle));

                    // FIXME event is not triggered
                }
            }
        }
    }

    pub fn uncell(&mut self) -> &mut World {
        self.world
    }

    pub fn entry<T: ?Sized>(&self, handle: ElementHandle<T>) -> Option<WorldCellEntry<'_, T>> {
        if !(self.contains(handle.untyped()) || self.inserted.borrow().contains(&handle.untyped()))
        {
            return None;
        }

        Some(WorldCellEntry {
            world: self,
            handle,
        })
    }
}
impl<T: Element> WorldEntry<'_, T> {
    pub fn fetch(&self) -> Option<&T> {
        self.world.fetch(self.handle)
    }

    pub fn fetch_mut(&mut self) -> Option<&mut T> {
        self.world.fetch_mut(self.handle)
    }

    pub fn getter<P: 'static>(&mut self, getter: impl Fn(&T) -> P + 'static) {
        match self.world.single_fetch_mut::<PropertyGetter<P>>() {
            Some(service) => {
                let ret = service.0.insert(
                    self.handle.untyped(),
                    Box::new(move |raw| getter(raw.downcast_ref::<T>().unwrap())),
                );

                if ret.is_some() {
                    log::error!(
                        "duplicated property getter of {} registered on {:?}!",
                        type_name::<P>(),
                        self.handle
                    );
                }
            }
            None => {
                let mut service = PropertyGetter::<P>(HashMap::new());

                service.0.insert(
                    self.handle.untyped(),
                    Box::new(move |raw| getter(raw.downcast_ref::<T>().unwrap())),
                );
                self.world.insert(service);

                log::trace!("property getter of {} is registered", type_name::<P>());
            }
        }
    }

    pub fn setter<P: 'static>(&mut self, setter: impl Fn(&mut T, P) + 'static) {
        match self.world.single_fetch_mut::<PropertySetter<P>>() {
            Some(service) => {
                let ret = service.0.insert(
                    self.handle.untyped(),
                    Box::new(move |raw, val| setter(raw.downcast_mut::<T>().unwrap(), val)),
                );

                if ret.is_some() {
                    log::error!(
                        "duplicated property setter of {} registered on {:?}!",
                        type_name::<P>(),
                        self.handle
                    );
                }
            }
            None => {
                let mut service = PropertySetter::<P>(HashMap::new());

                service.0.insert(
                    self.handle.untyped(),
                    Box::new(move |raw, val| setter(raw.downcast_mut::<T>().unwrap(), val)),
                );
                self.world.insert(service);

                log::trace!("property setter of {} is registered", type_name::<P>());
            }
        }
    }
}
impl<T: ?Sized> WorldEntry<'_, T> {
    pub fn observe<E: 'static>(
        &mut self,
        mut action: impl FnMut(&E, WorldCellEntry<T>) + 'static,
    ) -> ElementHandle {
        let this = self.handle().untyped();
        let handle = self.world.insert(Observer {
            action: Box::new(move |event, world| {
                if let Some(entry) = world.entry(this) {
                    action(event, entry.cast());
                }
            }),
            target: self.handle.untyped(),
        });

        match self.world.single_fetch_mut::<Observers<E>>() {
            Some(observers) => {
                let observers = observers.members.entry(self.handle.untyped()).or_default();
                observers.push(handle);
            }
            None => {
                let mut observers = Observers::<E> {
                    members: HashMap::new(),
                };
                let observer = observers.members.entry(self.handle.untyped()).or_default();
                observer.push(handle);
                self.insert(observers);
            }
        }

        self.world
            .entry(handle.untyped())
            .unwrap()
            .depend(self.handle.untyped());

        handle.untyped()
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let world = self.world.cell();
        if let Some(observers) = world.single_fetch::<Observers<E>>()
            && let Some(observers) = observers.members.get(&self.handle.untyped())
        {
            for observer in observers {
                if let Some(mut observer) = world.fetch_mut(*observer) {
                    let observer = &mut *observer;
                    (observer.action)(event, &world);
                }
            }
        }
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, depend_on: ElementHandle) {
        let depend_by = self.handle.untyped();
        if !self.world.contains(depend_on) {
            log::error!("{depend_by:?} try to depend on {depend_on:?}, which does not exist");
            return;
        }

        match self.world.single_fetch_mut::<Dependencies>() {
            Some(dependencies) => {
                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(depend_by);
                let depend = dependencies.0.entry(depend_by).or_default();
                depend.depend_on.push(depend_on);
            }
            None => {
                let mut dependencies = Dependencies::default();
                let depend = dependencies.0.entry(depend_on).or_default();
                depend.depend_by.push(depend_by);
                let depend = dependencies.0.entry(depend_by).or_default();
                depend.depend_on.push(depend_on);
                self.world.insert(dependencies);
            }
        }
    }

    pub fn destroy(self) {
        self.world.remove(self.handle.untyped());
    }

    pub fn handle(&self) -> ElementHandle<T> {
        self.handle
    }

    pub fn world(&mut self) -> &mut World {
        self.world
    }

    pub fn other<U: ?Sized>(&mut self, other: ElementHandle<U>) -> Option<WorldOther<'_, T, U>> {
        if !(self.contains(other.untyped())) {
            return None;
        }

        Some(WorldOther {
            world: self.world,
            handle: self.handle,
            other,
        })
    }

    pub fn untyped(&mut self) -> WorldEntry<'_, dyn Element> {
        self.cast()
    }

    fn cast<U: ?Sized>(&mut self) -> WorldEntry<'_, U> {
        WorldEntry {
            world: self.world,
            handle: self.handle.cast(),
        }
    }
}
impl<T: Element> WorldCellEntry<'_, T> {
    pub fn fetch(&self) -> Option<Ref<'_, T>> {
        self.world.fetch(self.handle)
    }

    pub fn fetch_mut(&self) -> Option<RefMut<'_, T>> {
        self.world.fetch_mut(self.handle)
    }

    /// This will be delayed until the cell is closed.
    pub fn getter<P: 'static>(&self, getter: impl Fn(&T) -> P + 'static) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.getter(getter);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn setter<P: 'static>(&self, setter: impl Fn(&mut T, P) + 'static) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.setter(setter);
        }));
    }
}
impl<T: ?Sized> WorldCellEntry<'_, T> {
    pub fn fetch_dyn(&self) -> Option<Ref<'_, dyn Element>> {
        self.world.fetch_dyn(self.handle.untyped())
    }

    pub fn fetch_mut_dyn(&self) -> Option<RefMut<'_, dyn Element>> {
        self.world.fetch_mut_dyn(self.handle.untyped())
    }

    pub fn service<U: ?Sized + 'static>(&self) -> Option<ServiceRef<'_, U>> {
        let services = self.single_fetch::<Services<U>>()?;
        if !services.0.contains_key(&self.handle.untyped()) {
            return None;
        }
        let element = self.fetch_dyn()?;
        Some(ServiceRef {
            services,
            element,
            handle: self.handle.untyped(),
        })
    }

    pub fn service_mut<U: ?Sized + 'static>(&self) -> Option<ServiceRefMut<'_, U>> {
        let services = self.single_fetch_mut::<Services<U>>()?;
        if !services.0.contains_key(&self.handle.untyped()) {
            return None;
        }
        let element = self.fetch_mut_dyn()?;
        Some(ServiceRefMut {
            services,
            element,
            handle: self.handle.untyped(),
        })
    }

    pub fn register<U: ?Sized + 'static>(&self, service: impl Service<U> + 'static) {
        if self.single::<Services<U>>().is_none() {
            self.insert(Services::<U>(HashMap::new()));
        }

        let mut services = self.single_fetch_mut::<Services<U>>().unwrap();
        services.0.insert(self.handle.untyped(), Box::new(service));
    }

    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(
        &self,
        mut action: impl FnMut(&E, WorldCellEntry<T>) + 'static,
    ) -> ElementHandle {
        let this = self.handle.untyped();
        let estimate_handle = self.world.insert(Observer {
            action: Box::new(move |event, world| {
                if let Some(entry) = world.entry(this) {
                    action(event, entry.cast());
                }
            }),
            target: this,
        });

        self.world
            .entry(estimate_handle.untyped())
            .unwrap()
            .depend(this);

        // observer will be registered in queue to prevent that some event triggered
        // before the insertion above hasn't even done yet
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            match world.single_fetch_mut::<Observers<E>>() {
                Some(observers) => {
                    let observers = observers.members.entry(this).or_default();
                    observers.push(estimate_handle);
                }
                None => {
                    let mut observers = Observers::<E> {
                        members: HashMap::new(),
                    };
                    let observer = observers.members.entry(this).or_default();
                    observer.push(estimate_handle);
                    world.insert(observers);
                }
            }
        }));

        estimate_handle.untyped()
    }

    /// This will be delayed until the cell is closed. So not all observers in the cell scope could receive the
    /// trigger (by its triggering order instead).
    ///
    /// This function has some limit since the event is delayed until cell closed, thus acquiring the ownership
    /// of the event.
    pub fn trigger<E: 'static>(&self, event: E) {
        let handle = self.handle.untyped();
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.trigger(&event);
        }));
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&self, depend_on: ElementHandle) {
        let handle = self.handle.untyped();
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.depend(depend_on);
        }));
    }

    pub fn queue(&self, f: impl FnOnce(WorldEntry<T>) + 'static)
    where
        T: 'static,
    {
        let handle = self.handle;
        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            if let Some(entry) = world.entry(handle) {
                f(entry);
            } else {
                log::error!("queued entry action for {handle:?} cannot access its target element");
            }
        }));
    }

    pub fn destroy(self) {
        self.world.remove(self.handle.untyped());
    }

    pub fn handle(&self) -> ElementHandle<T> {
        self.handle
    }

    pub fn other<U: ?Sized>(&self, other: ElementHandle<U>) -> Option<WorldCellOther<'_, T, U>> {
        if !(self.contains(other.untyped()) || self.inserted.borrow().contains(&other.untyped())) {
            return None;
        }

        Some(WorldCellOther {
            world: self.world,
            handle: self.handle,
            other,
        })
    }

    pub fn single_other<U: Element>(&self) -> Option<WorldCellOther<'_, T, U>> {
        self.other(self.world.single::<U>()?)
    }

    pub fn world(&self) -> &WorldCell<'_> {
        self.world
    }

    pub fn untyped(&self) -> WorldCellEntry<'_, dyn Element> {
        self.cast()
    }

    fn cast<U: ?Sized>(&self) -> WorldCellEntry<'_, U> {
        WorldCellEntry {
            world: self.world,
            handle: self.handle.cast(),
        }
    }
}
impl<T: ?Sized, U: ?Sized> WorldOther<'_, T, U> {
    /// Will depend on both.
    ///
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(
        &mut self,
        mut action: impl FnMut(&E, WorldCellEntry<T>) + 'static,
    ) -> ElementHandle {
        let this = self.handle.untyped();
        let other = self.other.untyped();
        let handle = self.world.insert(Observer {
            action: Box::new(move |event, world| {
                if let Some(entry) = world.entry(this) {
                    action(event, entry.cast());
                }
            }),
            target: other,
        });

        match self.world.single_fetch_mut::<Observers<E>>() {
            Some(observers) => {
                let observers = observers.members.entry(other).or_default();
                observers.push(handle);
            }
            None => {
                let mut observers = Observers::<E> {
                    members: HashMap::new(),
                };
                let observer = observers.members.entry(other).or_default();
                observer.push(handle);
                self.world.insert(observers);
            }
        }

        self.world.entry(handle.untyped()).unwrap().depend(this);
        self.world.entry(handle.untyped()).unwrap().depend(other);

        handle.untyped()
    }

    pub fn handle(&self) -> ElementHandle<T> {
        self.handle
    }

    pub fn other(&self) -> ElementHandle<U> {
        self.other
    }

    pub fn world(&mut self) -> &mut World {
        self.world
    }

    pub fn entry(&mut self) -> WorldEntry<'_, T> {
        WorldEntry {
            world: self.world,
            handle: self.handle,
        }
    }
}
impl<T: ?Sized, U: ?Sized> WorldCellOther<'_, T, U> {
    /// Will depend on both.
    ///
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(
        &self,
        mut action: impl FnMut(&E, WorldCellEntry<T>) + 'static,
    ) -> ElementHandle {
        let this = self.handle.untyped();
        let other = self.other.untyped();
        let estimate_handle = self.world.insert(Observer {
            action: Box::new(move |event, world| {
                if let Some(entry) = world.entry(this) {
                    action(event, entry.cast());
                }
            }),
            target: other,
        });

        self.world
            .entry(estimate_handle.untyped())
            .unwrap()
            .depend(this);
        self.world
            .entry(estimate_handle.untyped())
            .unwrap()
            .depend(other);

        // observer will be registered in queue to prevent that some event triggered
        // before the insertion above hasn't even done yet
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            match world.single_fetch_mut::<Observers<E>>() {
                Some(observers) => {
                    let observers = observers.members.entry(other).or_default();
                    observers.push(estimate_handle);
                }
                None => {
                    let mut observers = Observers::<E> {
                        members: HashMap::new(),
                    };
                    let observer = observers.members.entry(other).or_default();
                    observer.push(estimate_handle);
                    world.insert(observers);
                }
            }
        }));

        estimate_handle.untyped()
    }

    pub fn handle(&self) -> ElementHandle<T> {
        self.handle
    }

    pub fn other(&self) -> ElementHandle<U> {
        self.other
    }

    pub fn world(&self) -> &WorldCell<'_> {
        self.world
    }

    pub fn entry(&self) -> WorldCellEntry<'_, T> {
        WorldCellEntry {
            world: self.world,
            handle: self.handle,
        }
    }
}

impl<T: ?Sized> Deref for WorldEntry<'_, T> {
    type Target = World;
    fn deref(&self) -> &Self::Target {
        self.world
    }
}
impl<T: ?Sized> DerefMut for WorldEntry<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
impl<'world, T: ?Sized> Deref for WorldCellEntry<'world, T> {
    type Target = WorldCell<'world>;
    fn deref(&self) -> &Self::Target {
        self.world
    }
}

/// A world's immutable element reference.
pub struct Ref<'world, T: ?Sized> {
    ptr: *const T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle<T>,
}

/// A world's limitedly mutable element reference.
pub struct RefMut<'world, T: ?Sized> {
    ptr: *mut T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle<T>,
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
        let cnt = occupied.get_mut(&self.handle.untyped()).unwrap();
        *cnt -= 1;
    }
}
impl<T: ?Sized> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.borrow_mut();
        let cnt = occupied.get_mut(&self.handle.untyped()).unwrap();
        *cnt += 1;
    }
}

/// A service immutable reference.
pub struct ServiceRef<'world, T: ?Sized> {
    services: Ref<'world, Services<T>>,
    element: Ref<'world, dyn Element>,
    handle: ElementHandle,
}

/// A service mutable reference.
pub struct ServiceRefMut<'world, T: ?Sized> {
    services: RefMut<'world, Services<T>>,
    element: RefMut<'world, dyn Element>,
    handle: ElementHandle,
}

impl<T: ?Sized> Deref for ServiceRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let service = self.services.0.get(&self.handle).unwrap();
        service.action(&*self.element)
    }
}

impl<T: ?Sized> Deref for ServiceRefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let service = self.services.0.get(&self.handle).unwrap();
        service.action(&*self.element)
    }
}

impl<T: ?Sized> DerefMut for ServiceRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let service = self.services.0.get_mut(&self.handle).unwrap();
        service.action_mut(&mut *self.element)
    }
}

// Internal Elements //

// observer & trigger
// FIXME observer cleanup
#[derive(Default)]
struct Observers<E> {
    members: HashMap<ElementHandle, SmallVec<[ElementHandle<Observer<E>>; 1]>>,
}
#[expect(clippy::type_complexity)]
struct Observer<E> {
    action: Box<dyn FnMut(&E, &WorldCell)>,
    target: ElementHandle,
}
impl<E: 'static> Element for Observers<E> {}
impl<E: 'static> Element for Observer<E> {}
impl<E: 'static> InsertElement for Observers<E> {}
impl<E: 'static> InsertElement for Observer<E> {}

// cell queue
#[derive(Default)]
#[expect(clippy::type_complexity)]
struct Queue(Vec<Box<dyn FnOnce(&mut World)>>);
impl Element for Queue {}
impl InsertElement for Queue {}

// depend
#[derive(Default)]
struct Dependencies(HashMap<ElementHandle, Dependence>);
#[derive(Default)]
struct Dependence {
    depend_on: SmallVec<[ElementHandle; 1]>,
    depend_by: SmallVec<[ElementHandle; 4]>,
}
impl Element for Dependencies {}
impl InsertElement for Dependencies {}

// property
// FIXME property cleanup
type Getter<P> = HashMap<ElementHandle, Box<dyn for<'a> Fn(&'a dyn Element) -> P>>;
type Setter<P> = HashMap<ElementHandle, Box<dyn Fn(&mut dyn Element, P)>>;
struct PropertyGetter<P>(Getter<P>);
struct PropertySetter<P>(Setter<P>);
impl<P: 'static> Element for PropertyGetter<P> {}
impl<P: 'static> Element for PropertySetter<P> {}
impl<P: 'static> InsertElement for PropertyGetter<P> {}
impl<P: 'static> InsertElement for PropertySetter<P> {}

// services //

/// services represent an object attached to it
pub trait Service<T: ?Sized> {
    fn action<'w>(&'w self, element: &'w dyn Element) -> &'w T;
    fn action_mut<'w>(&'w mut self, element: &'w mut dyn Element) -> &'w mut T;
}

impl<
    T: ?Sized + 'static,
    F: for<'world> Fn(&'world dyn Element) -> &'world T,
    FM: for<'world> FnMut(&'world mut dyn Element) -> &'world mut T,
> Service<T> for (F, FM)
{
    fn action<'w>(&'w self, element: &'w dyn Element) -> &'w T {
        (self.0)(element)
    }

    fn action_mut<'w>(&'w mut self, element: &'w mut dyn Element) -> &'w mut T {
        (self.1)(element)
    }
}

struct Services<T: ?Sized>(HashMap<ElementHandle, Box<dyn Service<T>>>);
impl<T: ?Sized + 'static> Element for Services<T> {}
impl<T: ?Sized + 'static> InsertElement for Services<T> {}

// Builtin Events //

pub struct PropertyChanged<P>(pub P);
pub struct Destroy;

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct TestInserter(usize);
    impl Element for TestInserter {}
    impl InsertElement for TestInserter {}

    #[derive(Debug, PartialEq, Eq)]
    struct TestGoodInserter(usize);
    impl Element for TestGoodInserter {}
    impl InsertElement for TestGoodInserter {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestEvent(usize);

    #[derive(Debug)]
    struct SingletonBoard;
    impl Element for SingletonBoard {}
    impl InsertElement for SingletonBoard {}

    #[test]
    fn basic() {
        let mut world = World::default();

        assert_eq!(world.single::<TestInserter>(), None);

        let tester1 = world.insert(TestInserter(0xFC01));

        assert_eq!(world.single::<TestInserter>(), Some(tester1));

        let tester2 = world.insert(TestInserter(0xFF02));

        assert_eq!(world.single::<TestInserter>(), None);
        assert_eq!(world.fetch(tester1).unwrap().0, 0xFC01);

        let ret = world.remove(tester1.untyped()).unwrap();

        assert!(ret.is::<TestInserter>());
        assert_eq!(ret.downcast_ref::<TestInserter>().unwrap().0, 0xFC01);
        assert_eq!(world.single::<TestInserter>(), Some(tester2));
        assert_eq!(world.single_fetch::<TestInserter>().unwrap().0, 0xFF02);

        let tester2 = world.fetch_mut(tester2).unwrap();
        tester2.0 = 0xFA09;

        assert_eq!(world.single_fetch::<TestInserter>().unwrap().0, 0xFA09);
    }

    #[test]
    fn cell() {
        let mut world = World::default();
        let mut world = world.cell();

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

        world.remove(tester3h.untyped());

        assert!(!world.contains(tester3h.untyped()));
        assert_eq!(world.fetch(tester1h).unwrap().0, 0xFB03);
        assert_eq!(world.fetch(tester2h).unwrap().0, 0xCC02);
    }

    #[test]
    #[should_panic = "is mutably borrowed"]
    fn cell_runtime_borrow_panic() {
        let mut world = World::default();
        let tester1h = world.insert(TestInserter(0xFC01));
        let world = world.cell();

        let _inserter1 = world.fetch_mut(tester1h).unwrap();
        let _inserter2 = world.fetch(tester1h).unwrap();
    }

    #[test]
    fn cell_runtime_borrow_conflict() {
        let mut world = World::default();
        let tester1h = world.insert(TestInserter(0xFC01));
        let world = world.cell();

        {
            assert!(!world.occupied(tester1h.untyped()));
            assert!(!world.occupied_mut(tester1h.untyped()));
        }

        {
            let _inserter1 = world.fetch_mut(tester1h).unwrap();

            assert!(world.occupied(tester1h.untyped()));
            assert!(world.occupied_mut(tester1h.untyped()));
        }

        {
            let _inserter1 = world.fetch(tester1h).unwrap();

            assert!(!world.occupied(tester1h.untyped()));
            assert!(world.occupied_mut(tester1h.untyped()));
        }
    }

    #[test]
    fn loop_dependency() {
        let mut world = World::default();

        let left = world.insert(TestInserter(1));
        let right = world.insert(TestGoodInserter(2));
        let right_now = world.insert(TestGoodInserter(3));
        let but = world.insert(TestInserter(4));

        world.entry(left).unwrap().depend(right.untyped());
        world.entry(right).unwrap().depend(left.untyped());
        world.entry(right_now).unwrap().depend(right.untyped());

        world.remove(left.untyped());

        assert!(!world.contains(left.untyped()));
        assert!(!world.contains(right.untyped()));
        assert!(!world.contains(right_now.untyped()));
        assert!(world.contains(but.untyped()));
    }

    #[test]
    fn observers() {
        let mut world = World::default();

        let left = world.insert(TestInserter(1));
        let right = world.insert(TestGoodInserter(2));

        world.entry(left).unwrap().observe(|TestEvent(i), entry| {
            let mut this = entry.fetch_mut().unwrap();
            this.0 += i;
        });

        world
            .entry(right)
            .unwrap()
            .other(left)
            .unwrap()
            .observe(|TestEvent(i), entry| {
                let mut this = entry.fetch_mut().unwrap();
                this.0 += i;
            });

        world.entry(left).unwrap().trigger(&TestEvent(10));

        assert_eq!(world.fetch(left).unwrap(), &TestInserter(11));
        assert_eq!(world.fetch(right).unwrap(), &TestGoodInserter(12));
    }
}
