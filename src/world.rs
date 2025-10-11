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
        self.trigger(&Destroy);
    }
}
impl Drop for WorldCell<'_> {
    fn drop(&mut self) {
        self.world.curr_idx = *self.cell_idx.get_mut();

        let queue = self.world.single_mut::<Queue>().unwrap();
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

        // update cache
        let cache = self.cache.entry(TypeId::of::<T>()).or_default();
        cache.insert(handle);

        // when_inserted
        let cell = self.cell();
        let mut element = cell.fetch_mut::<T>(handle).unwrap();
        element.when_inserted(cell.entry(handle).unwrap());
        drop(element);
        drop(cell);

        // ElementInserted
        self.trigger(&ElementInserted(handle));

        log::trace!("insert {}: {:?}", type_name::<T>(), handle);
        handle
    }

    pub fn remove(&mut self, handle: ElementHandle) -> Option<Box<dyn Element>> {
        let type_id = (**self.elements.get(&handle)?).type_id();

        // remove children first
        let depend = self.single::<Dependencies>().unwrap();
        if let Some(children) = depend.cache.get(&handle) {
            for child in children.clone() {
                self.remove(child);
            }
        }

        // trigger events
        self.entry(handle).unwrap().trigger(&Destroy);
        self.trigger(&ElementRemoved(handle));

        // update cache
        let cache = self.cache.entry(type_id).or_default();
        cache.remove(&handle);

        // clean dependence to parent
        let depend = self.single_mut::<Dependencies>().unwrap();
        if let Some(parents) = depend.real.remove(&handle) {
            for parent in parents {
                let parent_children = depend.cache.get_mut(&parent).unwrap();
                for i in 0..parent_children.len() {
                    if parent_children[i] == handle {
                        parent_children.swap_remove(i);
                        break;
                    }
                }
            }
        }

        // TODO RemovalCapture(Box<dyn Element>)
        log::trace!("remove {:?}", handle);
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
    pub fn single<T: Element>(&self) -> Option<&T> {
        let mut iter = self.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        self.fetch(*ret)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&mut self) -> Option<&mut T> {
        let mut iter = self.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        self.fetch_mut(*ret)
    }

    pub fn get<T: 'static>(&self, handle: ElementHandle) -> Option<T> {
        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle)?.as_ref();
        Some(getter(element))
    }

    pub fn set<T: 'static>(&mut self, handle: ElementHandle, value: T) -> Option<()> {
        let setter = *self.single::<PropertySetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get_mut(&handle)?.as_mut();

        setter(element, value);

        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle).unwrap().as_ref();

        self.trigger(&ModifiedProperty(getter(element)));

        Some(())
    }

    pub fn modify<T: 'static>(&self, handle: ElementHandle) -> Option<Modify<T>> {
        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.elements.get(&handle)?.as_ref();

        if !self.single::<PropertySetter<T>>()?.0.contains_key(&handle) {
            return None;
        }

        Some(Modify {
            target: handle,
            value: getter(element),
        })
    }

    pub fn cell(&mut self) -> WorldCell<'_> {
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

    // TODO remove this when lnwin refactor is done

    /// Notice that it's *NOT* observing events world-wide! It's only observe events triggered also
    /// directly on world, which is useful when you don't have a specific element to attach the event.
    pub fn observe<E: 'static>(
        &mut self,
        mut action: impl FnMut(&E, &WorldCell) + 'static,
    ) -> ElementHandle {
        (self.entry(ElementHandle(0)).unwrap()).observe(move |event, entry| {
            action(event, entry.world);
        })
    }

    /// Will only trigger the observers mounted on the world. See [`World::observer`] for more.
    pub fn trigger<E: 'static>(&mut self, event: &E) {
        self.entry(ElementHandle(0)).unwrap().trigger(event);
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

        let mut inserted = self.inserted.borrow_mut();
        inserted.insert(estimate_handle);

        let mut queue = self.single_mut::<Queue>().unwrap();
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

            // ElementInserted
            world.trigger(&ElementInserted(estimate_handle));
        }));

        log::trace!("insert {}: {:?}", type_name::<T>(), estimate_handle);
        estimate_handle
    }

    /// Cell-mode removal cannot access the element immediately so we can't return the value of removed element.
    pub fn remove(&self, handle: ElementHandle) -> usize {
        if !self.contains(handle) {
            return 0;
        }

        let type_id = (**self.world.elements.get(&handle).unwrap()).type_id();

        let mut cnt = 0;

        // remove children first
        let depend = self.single::<Dependencies>().unwrap();
        if let Some(children) = depend.cache.get(&handle) {
            for child in children.clone() {
                cnt += self.remove(child);
            }
        }

        let mut queue = self.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            // trigger events
            world.entry(handle).unwrap().trigger(&Destroy);
            world.trigger(&ElementRemoved(handle));

            // update cache
            let cache = world.cache.entry(type_id).or_default();
            cache.remove(&handle);

            // clean dependence to parent
            let depend = world.single_mut::<Dependencies>().unwrap();
            if let Some(parents) = depend.real.remove(&handle) {
                for parent in parents {
                    let parent_children = depend.cache.get_mut(&parent).unwrap();
                    for i in 0..parent_children.len() {
                        if parent_children[i] == handle {
                            parent_children.swap_remove(i);
                            break;
                        }
                    }
                }
            }

            // TODO RemovalCapture(Box<dyn Element>)
            world.elements.remove(&handle);
        }));

        log::trace!("remove {:?}", handle);
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
    pub fn single<T: Element>(&self) -> Option<Ref<'_, T>> {
        let mut iter = self.world.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        self.fetch(*ret)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        let mut iter = self.world.cache.get(&TypeId::of::<T>())?.iter();
        let ret = iter.next()?;
        if iter.next().is_some() {
            return None;
        }
        self.fetch_mut(*ret)
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

        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
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

        let setter = *self.single::<PropertySetter<T>>()?.0.get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();

        let element_ptr = element as *const dyn Element as *mut dyn Element;
        setter(unsafe { element_ptr.as_mut().unwrap() }, value);

        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.world.elements.get(&handle).unwrap().as_ref();

        self.trigger(ModifiedProperty(getter(element)));

        Some(())
    }

    pub fn modify<T: 'static>(&self, handle: ElementHandle) -> Option<Modify<T>> {
        if self.removed.borrow().contains(&handle) {
            return None;
        }

        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        let getter = *self.single::<PropertyGetter<T>>()?.0.get(&handle)?;
        let element = self.world.elements.get(&handle)?.as_ref();

        if !self.single::<PropertySetter<T>>()?.0.contains_key(&handle) {
            return None;
        }

        Some(Modify {
            target: handle,
            value: getter(element),
        })
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

    // TODO remove this when lnwin refactor is done

    /// Notice that it's *NOT* observing events world-wide! It's only observe events triggered also
    /// directly on world, which is useful when you don't have a specific element to attach the event.
    ///
    /// This will be delayed until the cell is closed.
    pub fn observe<E: 'static>(
        &self,
        mut action: impl FnMut(&E, &WorldCell) + 'static,
    ) -> ElementHandle {
        (self.entry(ElementHandle(0)).unwrap()).observe(move |event, entry| {
            action(event, entry.world);
        })
    }

    /// Will only trigger the observers mounted on the world. See [`WorldCell::observer`] for more.
    ///
    /// This function has some limit since the event is delayed until cell closed, thus acquiring the ownership
    /// of the event.
    pub fn trigger<E: 'static>(&self, event: E) {
        self.entry(ElementHandle(0)).unwrap().trigger(event);
    }
}
impl WorldEntry<'_> {
    pub fn observe<E: 'static>(
        &mut self,
        mut action: impl FnMut(&E, WorldCellEntry) + 'static,
    ) -> ElementHandle {
        let this = self.handle;
        let handle = self.world.insert(Observer(Box::new(move |event, world| {
            let event = event.downcast_ref::<E>().unwrap();
            let entry = world.entry(this).unwrap();
            action(event, entry);
        })));

        match self.world.single_mut::<Observers<E>>() {
            Some(observers) => {
                let observers = observers.0.entry(self.handle).or_default();
                observers.push(handle);
            }
            None => {
                let mut observers = Observers::<E>(HashMap::new(), PhantomData);
                let observers = observers.0.entry(self.handle).or_default();
                observers.push(handle);
            }
        }

        self.world.entry(handle).unwrap().depend(self.handle);

        handle
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let cell = self.world.cell();
        if let Some(observers) = cell.single::<Observers<E>>()
            && let Some(observers) = observers.0.get(&self.handle)
        {
            for observer in observers {
                if let Some(mut observer) = cell.fetch_mut::<Observer>(*observer) {
                    (observer.0)(event, &cell);
                }
            }
        }
    }

    pub fn getter<T: 'static>(&mut self, getter: fn(&dyn Element) -> T) {
        match self.world.single_mut::<PropertyGetter<T>>() {
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
        match self.world.single_mut::<PropertySetter<T>>() {
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
    pub fn depend(&mut self, parent: ElementHandle) {
        let child = self.handle;
        if !self.world.contains(parent) {
            log::error!("{child:?} try to depend on {parent:?}, which does not exist");
            return;
        }

        let depend = self.world.single_mut::<Dependencies>().unwrap();
        depend.real.entry(child).or_default().push(parent);
        depend.cache.entry(parent).or_default().push(child);
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
        mut action: impl FnMut(&E, WorldCellEntry) + 'static,
    ) -> ElementHandle {
        let this = self.handle;
        let estimate_handle = self.world.insert(Observer(Box::new(move |event, world| {
            let event = event.downcast_ref::<E>().unwrap();
            let entry = world.entry(this).unwrap();
            action(event, entry);
        })));

        // observer will be registered in queue to prevent that some event triggered
        // before the insertion above hasn't even done yet
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            match world.single_mut::<Observers<E>>() {
                Some(observers) => {
                    let observers = observers.0.entry(this).or_default();
                    observers.push(estimate_handle);
                }
                None => {
                    let mut observers = Observers::<E>(HashMap::new(), PhantomData);
                    let observers = observers.0.entry(this).or_default();
                    observers.push(estimate_handle);
                }
            }

            world.entry(estimate_handle).unwrap().depend(this);
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
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.trigger(&event);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn getter<T: 'static>(&mut self, getter: fn(&dyn Element) -> T) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.getter(getter);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn setter<T: 'static>(&mut self, setter: fn(&mut dyn Element, T)) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.setter(setter);
        }));
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, parent: ElementHandle) {
        let child = self.handle;
        if !(self.world.contains(parent) || self.world.inserted.borrow().contains(&parent)) {
            log::error!("{child:?} try to depend on {parent:?}, which does not exist");
            return;
        }

        let mut depend = self.world.single_mut::<Dependencies>().unwrap();
        depend.real.entry(child).or_default().push(parent);
        depend.cache.entry(parent).or_default().push(child);
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

/// `Modify` is a helper for property that have both getter and setter
pub struct Modify<T> {
    target: ElementHandle,
    value: T,
}
impl<T> Deref for Modify<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> DerefMut for Modify<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}
impl<T: 'static> Modify<T> {
    pub fn reset(&mut self, world: &World) {
        let property = world.single::<PropertyGetter<T>>().unwrap();
        let getter = *property.0.get(&self.target).unwrap();
        let element = world.elements.get(&self.target).unwrap().as_ref();

        self.value = getter(element);
    }

    pub fn flush(self, world: &mut World) {
        let property = world.single::<PropertySetter<T>>().unwrap();
        let setter = *property.0.get(&self.target).unwrap();
        let element = world.elements.get_mut(&self.target).unwrap().as_mut();

        setter(element, self.value);

        let property = world.single::<PropertyGetter<T>>().unwrap();
        let getter = *property.0.get(&self.target).unwrap();
        let element = world.elements.get(&self.target).unwrap().as_ref();

        world.trigger(&ModifiedProperty(getter(element)));
    }
}

// Internal Elements //

// observer & trigger
// FIXME observer cleanup
#[derive(Default)]
struct Observers<E>(
    HashMap<ElementHandle, SmallVec<[ElementHandle; 1]>>,
    PhantomData<E>,
);
#[expect(clippy::type_complexity)]
struct Observer(Box<dyn FnMut(&dyn Any, &WorldCell)>);
impl<E: 'static> Element for Observers<E> {}
impl Element for Observer {}

// cell queue
#[derive(Default)]
#[expect(clippy::type_complexity)]
struct Queue(Vec<Box<dyn FnOnce(&mut World)>>);
impl Element for Queue {}

// TODO depend
#[derive(Default)]
struct Dependencies {
    // real: <child, parent>
    real: HashMap<ElementHandle, SmallVec<[ElementHandle; 1]>>,
    // cache: <parent, child>
    cache: HashMap<ElementHandle, SmallVec<[ElementHandle; 4]>>,
}
struct Dependence {
    depend_on: SmallVec<[ElementHandle; 1]>,
    depend_by: SmallVec<[ElementHandle; 4]>,
}
impl Element for Dependencies {}

// property & modify
// FIXME property cleanup
struct PropertyGetter<T>(HashMap<ElementHandle, fn(&dyn Element) -> T>);
struct PropertySetter<T>(HashMap<ElementHandle, fn(&mut dyn Element, T)>);
impl<T: 'static> Element for PropertyGetter<T> {}
impl<T: 'static> Element for PropertySetter<T> {}

// Builtin Events //

pub struct ElementInserted(pub ElementHandle);
pub struct ElementRemoved(pub ElementHandle);
pub struct ModifiedProperty<T>(pub T);
pub struct Destroy;
