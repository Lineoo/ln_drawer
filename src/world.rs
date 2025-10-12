use std::{
    any::{Any, TypeId, type_name},
    cell::RefCell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

/// A shared form of objects in the [`World`].
#[expect(unused_variables)]
pub trait Element: Any {
    fn when_inserted(&mut self, entry: WorldCellEntry) {}
}
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

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementHandle(usize);

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
pub struct WorldEntry<'world> {
    world: &'world mut World,
    handle: ElementHandle,
}

/// A world cell reference with specific element selected. No borrowing effect.
pub struct WorldCellEntry<'world> {
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
}

impl Default for World {
    fn default() -> Self {
        World {
            curr_idx: ElementHandle(0),
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
        self.world.curr_idx = *self.cell_idx.get_mut();

        let queue = self.world.single_fetch_mut::<Queue>().unwrap();
        let mut buf = Vec::new();
        buf.append(&mut queue.0);

        for cmd in buf {
            cmd(self.world);
        }
    }
}

impl World {
    pub fn insert<T: Element + 'static>(&mut self, element: T) -> ElementHandle {
        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        let handle = ElementHandle(self.curr_idx.0 - 1);
        log::trace!("insert {}: {:?}", type_name::<T>(), handle);

        // update cache
        let cache = self.cache.entry(TypeId::of::<T>()).or_default();
        cache.insert(handle);

        // when_inserted
        let cell = self.cell();
        let mut element = cell.fetch_mut::<T>(handle).unwrap();
        element.when_inserted(cell.entry(handle).unwrap());
        drop(element);
        drop(cell);

        handle
    }

    pub fn remove(&mut self, handle: ElementHandle) -> Option<Box<dyn Element>> {
        let type_id = (**self.elements.get(&handle)?).type_id();
        log::trace!("remove {:?}", handle);

        // remove children first
        if let Some(dependencies) = self.single_fetch::<Dependencies>()
            && let Some(this) = dependencies.0.get(&handle)
        {
            for child in this.depend_by.clone() {
                self.remove(child);
            }
        }

        // trigger events
        self.entry(handle).unwrap().trigger(&Destroy);

        // update cache
        let cache = self.cache.entry(type_id).or_default();
        cache.remove(&handle);

        // clean dependence to parent
        if let Some(dependencies) = self.single_fetch_mut::<Dependencies>()
            && let Some(this) = dependencies.0.get(&handle)
        {
            for parent in this.depend_on.clone() {
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

            dependencies.0.remove(&handle);
        }

        // TODO RemovalCapture(Box<dyn Element>)
        self.elements.remove(&handle)
    }

    pub fn contains(&self, handle: ElementHandle) -> bool {
        self.elements.contains_key(&handle)
    }

    pub fn contains_type<T: Element>(&self, handle: ElementHandle) -> bool {
        self.elements
            .get(&handle)
            .is_some_and(|element| element.is::<T>())
    }

    pub fn fetch<T: Element>(&self, handle: ElementHandle) -> Option<&T> {
        self.elements
            .get(&handle)
            .and_then(|element| element.downcast_ref())
    }

    pub fn fetch_mut<T: Element>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        self.elements
            .get_mut(&handle)
            .and_then(|element| element.downcast_mut())
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<ElementHandle> {
        let mut iter = self.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(*ret)
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
    pub fn single_entry<T: Element>(&mut self) -> Option<WorldEntry<'_>> {
        self.entry(self.single::<T>()?)
    }

    pub fn get<T: 'static>(&self, handle: ElementHandle) -> Option<T> {
        let getter = *self.single_fetch::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle)?.as_ref();
        Some(getter(element))
    }

    pub fn set<T: 'static>(&mut self, handle: ElementHandle, value: T) -> Option<()> {
        let setter = *self.single_fetch::<PropertySetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get_mut(&handle)?.as_mut();

        setter(element, value);

        let getter = *self.single_fetch::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle).unwrap().as_ref();

        let value = getter(element);
        self.entry(handle)?.trigger(&PropertyChanged(value));

        Some(())
    }

    pub fn get_foreach<T: 'static>(&self, mut action: impl FnMut(T)) {
        if let Some(property) = self.single_fetch::<PropertyGetter<T>>() {
            for (&handle, &getter) in &property.0 {
                if let Some(element) = self.elements.get(&handle) {
                    action(getter(element.as_ref()));
                }
            }
        }
    }

    pub fn set_foreach<T: 'static>(&mut self, mut action: impl FnMut() -> T) {
        if let Some(property) = self.single_fetch::<PropertySetter<T>>() {
            for (handle, setter) in property.0.clone() {
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

    pub fn entry(&mut self, handle: ElementHandle) -> Option<WorldEntry<'_>> {
        if !self.contains(handle) {
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
    pub fn insert<T: Element + 'static>(&self, element: T) -> ElementHandle {
        // get estimate_handle
        // cell-mode insertion depends on *retained* handle
        let mut cell_idx = self.cell_idx.borrow_mut();
        cell_idx.0 += 1;
        let estimate_handle = ElementHandle(cell_idx.0 - 1);
        log::trace!("insert {}: {:?}", type_name::<T>(), estimate_handle);

        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(estimate_handle);

        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            world.elements.insert(estimate_handle, Box::new(element));

            // update cache
            let cache = world.cache.entry(TypeId::of::<T>()).or_default();
            cache.insert(estimate_handle);

            // when_inserted
            let cell = world.cell();
            let mut element = cell.fetch_mut::<T>(estimate_handle).unwrap();
            element.when_inserted(cell.entry(estimate_handle).unwrap());
            drop(element);
            drop(cell);
        }));

        estimate_handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the value of removed element.
    pub fn remove(&self, handle: ElementHandle) -> usize {
        if !self.contains(handle) {
            return 0;
        }

        let type_id = (**self.world.elements.get(&handle).unwrap()).type_id();
        log::trace!("remove {:?}", handle);

        let mut cnt = 1;

        // remove children first
        if let Some(dependencies) = self.single_fetch::<Dependencies>()
            && let Some(this) = dependencies.0.get(&handle)
        {
            for child in this.depend_by.clone() {
                cnt += self.remove(child);
            }
        }

        let mut queue = self.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            // trigger events
            world.entry(handle).unwrap().trigger(&Destroy);

            // update cache
            let cache = world.cache.entry(type_id).or_default();
            cache.remove(&handle);

            // clean dependence to parent
            if let Some(dependencies) = world.single_fetch_mut::<Dependencies>()
                && let Some(this) = dependencies.0.get(&handle)
            {
                for parent in this.depend_on.clone() {
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

                dependencies.0.remove(&handle);
            }

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

    /// Insertion happened within the cell scope will not be included
    pub fn contains(&self, handle: ElementHandle) -> bool {
        if self.removed.borrow().contains(&handle) {
            return false;
        }
        self.world.contains(handle)
    }

    /// Insertion happened within the cell scope will not be included
    pub fn contains_type<T: Element>(&self, handle: ElementHandle) -> bool {
        if self.removed.borrow().contains(&handle) {
            return false;
        }
        self.world.contains_type::<T>(handle)
    }

    pub fn fetch<T: Element>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle)?.downcast_ref()?;

        Some(Ref {
            ptr: element as *const T,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: ElementHandle) -> Option<RefMut<'_, T>> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle)?.downcast_ref()?;

        Some(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
        })
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<ElementHandle> {
        let mut iter = self.world.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        Some(*ret)
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
    pub fn single_entry<T: Element>(&self) -> Option<WorldCellEntry<'_>> {
        self.entry(self.single::<T>()?)
    }

    pub fn get<T: 'static>(&self, handle: ElementHandle) -> Option<T> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        let getter = *self
            .world
            .single_fetch::<PropertyGetter<T>>()?
            .0
            .get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();
        Some(getter(element))
    }

    pub fn set<T: 'static>(&self, handle: ElementHandle, value: T) -> Option<()> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        let setter = *self
            .world
            .single_fetch::<PropertySetter<T>>()?
            .0
            .get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();

        let element_ptr = element as *const dyn Element as *mut dyn Element;
        setter(unsafe { element_ptr.as_mut().unwrap() }, value);

        let getter = *self
            .world
            .single_fetch::<PropertyGetter<T>>()?
            .0
            .get(&handle)?;
        let element = self.world.elements.get(&handle).unwrap().as_ref();

        drop(occupied);

        self.entry(handle)?
            .trigger(PropertyChanged(getter(element)));

        Some(())
    }

    pub fn get_foreach<T: 'static>(&self, mut action: impl FnMut(ElementHandle, T)) {
        if let Some(property) = self.world.single_fetch::<PropertyGetter<T>>() {
            let mut occupied = self.occupied.borrow_mut();
            for (&handle, &getter) in &property.0 {
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

    pub fn set_foreach<T: 'static>(&mut self, mut action: impl FnMut(ElementHandle) -> T) {
        if let Some(property) = self.world.single_fetch::<PropertySetter<T>>() {
            let mut occupied = self.occupied.borrow_mut();
            for (handle, setter) in property.0.clone() {
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

    pub fn entry(&self, handle: ElementHandle) -> Option<WorldCellEntry<'_>> {
        if !(self.contains(handle) || self.inserted.borrow().contains(&handle)) {
            return None;
        }

        Some(WorldCellEntry {
            world: self,
            handle,
        })
    }
}
impl WorldEntry<'_> {
    pub fn observe<E: 'static>(
        &mut self,
        action: impl FnMut(&E, WorldCellEntry) + 'static,
    ) -> ElementHandle {
        let this = self.handle;
        let handle = self.world.insert(Observer {
            action: Box::new(action),
            target: this,
        });

        match self.world.single_fetch_mut::<Observers<E>>() {
            Some(observers) => {
                let observers = observers.observers.entry(self.handle).or_default();
                observers.push(handle);
            }
            None => {
                let mut observers = Observers::<E> {
                    observers: HashMap::new(),
                    _marker: PhantomData,
                };
                let observer = observers.observers.entry(self.handle).or_default();
                observer.push(handle);
                self.insert(observers);
            }
        }

        self.world.entry(handle).unwrap().depend(self.handle);

        handle
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let world = self.world.cell();
        if let Some(observers) = world.single_fetch::<Observers<E>>()
            && let Some(observers) = observers.observers.get(&self.handle)
        {
            for observer in observers {
                if let Some(mut observer) = world.fetch_mut::<Observer<E>>(*observer) {
                    let observer = &mut *observer;
                    (observer.action)(event, world.entry(observer.target).unwrap());
                }
            }
        }
    }

    pub fn getter<T: 'static>(&mut self, getter: fn(&dyn Element) -> T) {
        match self.world.single_fetch_mut::<PropertyGetter<T>>() {
            Some(service) => {
                let ret = service.0.insert(self.handle, getter);

                if ret.is_some() {
                    log::error!(
                        "duplicated property getter of {} registered on {:?}!",
                        type_name::<T>(),
                        self.handle
                    );
                }
            }
            None => {
                let mut service = PropertyGetter::<T>(HashMap::new());

                service.0.insert(self.handle, getter);
                self.world.insert(service);

                log::trace!("property getter of {} is registered", type_name::<T>());
            }
        }
    }

    pub fn setter<T: 'static>(&mut self, setter: fn(&mut dyn Element, T)) {
        match self.world.single_fetch_mut::<PropertySetter<T>>() {
            Some(service) => {
                let ret = service.0.insert(self.handle, setter);

                if ret.is_some() {
                    log::error!(
                        "duplicated property setter of {} registered on {:?}!",
                        type_name::<T>(),
                        self.handle
                    );
                }
            }
            None => {
                let mut service = PropertySetter::<T>(HashMap::new());

                service.0.insert(self.handle, setter);
                self.world.insert(service);

                log::trace!("property setter of {} is registered", type_name::<T>());
            }
        }
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, depend_on: ElementHandle) {
        let depend_by = self.handle;
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
        self.world.remove(self.handle);
    }

    pub fn handle(&self) -> ElementHandle {
        self.handle
    }

    pub fn world(&mut self) -> &mut World {
        self.world
    }
}
impl WorldCellEntry<'_> {
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(
        &mut self,
        action: impl FnMut(&E, WorldCellEntry) + 'static,
    ) -> ElementHandle {
        let this = self.handle;
        let estimate_handle = self.world.insert(Observer {
            action: Box::new(action),
            target: this,
        });

        // observer will be registered in queue to prevent that some event triggered
        // before the insertion above hasn't even done yet
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            match world.single_fetch_mut::<Observers<E>>() {
                Some(observers) => {
                    let observers = observers.observers.entry(this).or_default();
                    observers.push(estimate_handle);
                }
                None => {
                    let mut observers = Observers::<E> {
                        observers: HashMap::new(),
                        _marker: PhantomData,
                    };
                    let observer = observers.observers.entry(this).or_default();
                    observer.push(estimate_handle);
                    world.insert(observers);
                }
            }
        }));

        estimate_handle
    }

    /// This will be delayed until the cell is closed. So not all observers in the cell scope could receive the
    /// trigger (by its triggering order instead).
    ///
    /// This function has some limit since the event is delayed until cell closed, thus acquiring the ownership
    /// of the event.
    pub fn trigger<E: 'static>(&mut self, event: E) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.trigger(&event);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn getter<T: 'static>(&mut self, getter: fn(&dyn Element) -> T) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.getter(getter);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn setter<T: 'static>(&mut self, setter: fn(&mut dyn Element, T)) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.setter(setter);
        }));
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, depend_on: ElementHandle) {
        let handle = self.handle;
        let mut queue = self.world.single_fetch_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.depend(depend_on);
        }));
    }

    pub fn destroy(self) {
        self.world.remove(self.handle);
    }

    pub fn handle(&self) -> ElementHandle {
        self.handle
    }

    pub fn world(&self) -> &WorldCell<'_> {
        self.world
    }
}

impl Deref for WorldEntry<'_> {
    type Target = World;
    fn deref(&self) -> &Self::Target {
        self.world
    }
}
impl DerefMut for WorldEntry<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}
impl<'world> Deref for WorldCellEntry<'world> {
    type Target = WorldCell<'world>;
    fn deref(&self) -> &Self::Target {
        self.world
    }
}

/// A world's immutable element reference.
pub struct Ref<'world, T: ?Sized> {
    ptr: *const T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
}

/// A world's limitedly mutable element reference.
pub struct RefMut<'world, T: ?Sized> {
    ptr: *mut T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
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
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt -= 1;
    }
}
impl<T: ?Sized> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.borrow_mut();
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt += 1;
    }
}

// Internal Elements //

// observer & trigger
// FIXME observer cleanup
#[derive(Default)]
struct Observers<E> {
    observers: HashMap<ElementHandle, SmallVec<[ElementHandle; 1]>>,
    _marker: PhantomData<E>,
}
#[expect(clippy::type_complexity)]
struct Observer<E> {
    action: Box<dyn FnMut(&E, WorldCellEntry)>,
    target: ElementHandle,
}
impl<E: 'static> Element for Observers<E> {}
impl<E: 'static> Element for Observer<E> {}

// cell queue
#[derive(Default)]
#[expect(clippy::type_complexity)]
struct Queue(Vec<Box<dyn FnOnce(&mut World)>>);
impl Element for Queue {}

// depend
#[derive(Default)]
struct Dependencies(HashMap<ElementHandle, Dependence>);
#[derive(Default)]
struct Dependence {
    depend_on: SmallVec<[ElementHandle; 1]>,
    depend_by: SmallVec<[ElementHandle; 4]>,
}
impl Element for Dependencies {}

// property
// FIXME property cleanup
struct PropertyGetter<T>(HashMap<ElementHandle, fn(&dyn Element) -> T>);
struct PropertySetter<T>(HashMap<ElementHandle, fn(&mut dyn Element, T)>);
impl<T: 'static> Element for PropertyGetter<T> {}
impl<T: 'static> Element for PropertySetter<T> {}

// Builtin Events //

pub struct PropertyChanged<T>(pub T);
pub struct Destroy;