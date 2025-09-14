use std::{
    any::{Any, TypeId},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::elements::Element;

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementHandle(usize);

enum Singleton {
    Unique(ElementHandle),
    Multiple(usize),
}

pub struct World {
    curr_idx: ElementHandle,
    elements: HashMap<ElementHandle, Box<dyn Element>>,
    singletons: HashMap<TypeId, Singleton>,
}
impl Default for World {
    fn default() -> Self {
        let mut elements = HashMap::<_, Box<dyn Element>>::new();
        let mut singletons = HashMap::new();

        elements.insert(ElementHandle(0), Box::new(Observers(HashMap::new())));
        singletons.insert(
            TypeId::of::<Observers>(),
            Singleton::Unique(ElementHandle(0)),
        );

        elements.insert(ElementHandle(1), Box::new(Queue(Vec::new())));
        singletons.insert(TypeId::of::<Queue>(), Singleton::Unique(ElementHandle(1)));

        World {
            curr_idx: ElementHandle(2),
            elements,
            singletons,
        }
    }
}
impl World {
    pub fn insert(&mut self, element: impl Element + 'static) -> ElementHandle {
        let type_id = element.type_id();

        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        let handle = ElementHandle(self.curr_idx.0 - 1);

        // when_inserted
        let cell = self.cell();
        let mut element = cell.fetch_mut_dyn(handle).unwrap();
        element.when_inserted(handle, &cell);
        drop(element);

        // ElementInserted
        self.trigger(&ElementInserted(handle));

        // singleton cache
        self.singletons
            .entry(type_id)
            .and_modify(|status| match status {
                Singleton::Unique(_) => {
                    *status = Singleton::Multiple(2);
                }
                Singleton::Multiple(cnt) => {
                    *cnt += 1;
                }
            })
            .or_insert(Singleton::Unique(handle));

        handle
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

    pub fn fetch_dyn(&self, handle: ElementHandle) -> Option<&dyn Element> {
        self.elements.get(&handle).map(|element| element.as_ref())
    }

    pub fn fetch_mut<T: Element>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        self.elements
            .get_mut(&handle)
            .and_then(|element| element.downcast_mut())
    }

    pub fn fetch_mut_dyn(&mut self, handle: ElementHandle) -> Option<&mut dyn Element> {
        self.elements
            .get_mut(&handle)
            .map(|element| element.as_mut())
    }

    pub fn entry<T: Element>(&mut self, handle: ElementHandle) -> Option<WorldElement<'_, T>> {
        if !self.elements.contains_key(&handle) {
            return None;
        }

        Some(WorldElement {
            world: self,
            handle,
            _marker: PhantomData,
        })
    }

    pub fn entry_dyn(&mut self, handle: ElementHandle) -> Option<WorldElement<'_, dyn Element>> {
        if !self.elements.contains_key(&handle) {
            return None;
        }

        Some(WorldElement {
            world: self,
            handle,
            _marker: PhantomData,
        })
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<&T> {
        if let Some(Singleton::Unique(handle)) = self.singletons.get(&TypeId::of::<T>()) {
            self.elements.get(handle)?.downcast_ref()
        } else {
            None
        }
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&mut self) -> Option<&mut T> {
        if let Some(Singleton::Unique(handle)) = self.singletons.get(&TypeId::of::<T>()) {
            self.elements.get_mut(handle)?.downcast_mut()
        } else {
            None
        }
    }

    pub fn cell(&mut self) -> WorldCell<'_> {
        WorldCell {
            world: self,
            occupied: Mutex::new(HashMap::new()),
        }
    }

    /// Global trigger. Will trigger every element listening to this event.
    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let cell = self.cell();
        let mut observers = cell.single_mut::<Observers>().unwrap();
        for observers in observers.0.values_mut() {
            for observer in observers {
                observer(event, &cell);
            }
        }
    }

    pub fn elements<T: Element>(&self) -> impl Iterator<Item = &T> {
        (self.elements.values()).filter_map(|element| element.downcast_ref::<T>())
    }

    pub fn elements_mut<T: Element>(&mut self) -> impl Iterator<Item = &mut T> {
        (self.elements.values_mut()).filter_map(|element| element.downcast_mut::<T>())
    }

    pub fn elements_dyn(&self) -> impl Iterator<Item = &dyn Element> {
        self.elements.values().map(|elem| elem.as_ref())
    }

    pub fn elements_mut_dyn(&mut self) -> impl Iterator<Item = &mut dyn Element> {
        self.elements.values_mut().map(|elem| elem.as_mut())
    }
}

/// A full mutable world reference with specific element selected.
pub struct WorldElement<'world, T: ?Sized> {
    world: &'world mut World,
    handle: ElementHandle,
    _marker: PhantomData<T>,
}
impl<T: Element> Deref for WorldElement<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.world.fetch(self.handle).unwrap()
    }
}
impl<T: Element> DerefMut for WorldElement<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world.fetch_mut(self.handle).unwrap()
    }
}
impl Deref for WorldElement<'_, dyn Element> {
    type Target = dyn Element;
    fn deref(&self) -> &Self::Target {
        self.world.fetch_dyn(self.handle).unwrap()
    }
}
impl DerefMut for WorldElement<'_, dyn Element> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world.fetch_mut_dyn(self.handle).unwrap()
    }
}
impl<T: ?Sized> WorldElement<'_, T> {
    pub fn observe<E: 'static>(&mut self, mut action: impl FnMut(&E, &WorldCell) + 'static) {
        let observers = self.world.single_mut::<Observers>().unwrap();
        let observers = observers.0.entry(self.handle).or_default();
        observers.push(Box::new(move |event, world| {
            if let Some(event) = event.downcast_ref::<E>() {
                action(event, world);
            }
        }));
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let cell = self.world.cell();
        let mut observers = cell.single_mut::<Observers>().unwrap();
        if let Some(observers) = observers.0.get_mut(&self.handle) {
            for observer in observers {
                observer(event, &cell);
            }
        }
    }
}

// Center of multiple accesses in world, which also prevents constructional changes
pub struct WorldCell<'world> {
    world: &'world mut World,
    // TODO use RefCell to optimize single-threaded situation
    occupied: Mutex<HashMap<ElementHandle, isize>>,
}
impl WorldCell<'_> {
    pub fn fetch<T: Element>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
        let mut occupied = self.occupied.lock();

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

    pub fn fetch_dyn(&self, handle: ElementHandle) -> Option<Ref<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle)?.as_ref();

        Some(Ref {
            ptr: element as *const dyn Element,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut<T: Element>(&self, handle: ElementHandle) -> Option<RefMut<'_, T>> {
        let mut occupied = self.occupied.lock();

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

    pub fn fetch_mut_dyn(&self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle)?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
        })
    }

    // Singleton

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<Ref<'_, T>> {
        if let Some(Singleton::Unique(handle)) = self.world.singletons.get(&TypeId::of::<T>()) {
            self.fetch(*handle)
        } else {
            None
        }
    }

    pub fn entry<T: Element>(&self, handle: ElementHandle) -> Option<WorldCellElement<'_, T>> {
        if !self.world.elements.contains_key(&handle) {
            return None;
        }

        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle)?.downcast_ref()?;

        Some(WorldCellElement {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
            _marker: PhantomData,
        })
    }

    pub fn entry_dyn(&self, handle: ElementHandle) -> Option<WorldCellElement<'_, dyn Element>> {
        if !self.world.elements.contains_key(&handle) {
            return None;
        }

        let mut occupied = self.occupied.lock();
        let element = self.world.elements.get(&handle)?.as_ref();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;

        Some(WorldCellElement {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
            _marker: PhantomData,
        })
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        if let Some(Singleton::Unique(handle)) = self.world.singletons.get(&TypeId::of::<T>()) {
            self.fetch_mut(*handle)
        } else {
            None
        }
    }

    pub fn world(&mut self) -> &mut World {
        self.world
    }

    /// Global trigger. Will trigger every element listening to this event. This will be delayed
    /// until the cell is closed.
    ///
    /// This function has some limit since the event is delayed until cell closed, thus acquiring the ownership
    /// of the event.
    pub fn trigger<E: 'static>(&self, event: E) {
        let mut queue = self.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            world.trigger(&event);
        }));
    }

    // Direct occupation skipping the lock

    pub fn occupy<T: Element>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        self.world.elements.get_mut(&handle)?.downcast_mut()
    }

    pub fn occupy_dyn(&mut self, handle: ElementHandle) -> Option<&mut dyn Element> {
        self.world.elements.get_mut(&handle).map(|elm| elm.as_mut())
    }
}

/// A world's immutable element reference.
pub struct Ref<'world, T: ?Sized> {
    ptr: *const T,
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
impl<T: ?Sized> Drop for Ref<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.lock();
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt -= 1;
    }
}

/// A world's limitedly mutable element reference.
pub struct RefMut<'world, T: ?Sized> {
    ptr: *mut T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
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
impl<T: ?Sized> Drop for RefMut<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.lock();
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt += 1;
    }
}

/// A world cell reference with specific element selected. The borrowing behavior is the same as
/// mutably borrowing.
pub struct WorldCellElement<'world, T: ?Sized> {
    ptr: *mut T,
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
    _marker: PhantomData<T>,
}
impl<T: ?Sized> Deref for WorldCellElement<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}
impl<T: ?Sized> DerefMut for WorldCellElement<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_mut().unwrap() }
    }
}
impl<T: ?Sized> Drop for WorldCellElement<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.lock();
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt += 1;
    }
}
impl<T: Element> WorldCellElement<'_, T> {
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(&mut self, action: impl FnMut(&E, &WorldCell) + 'static) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry::<T>(handle).unwrap();
            this.observe(action);
        }));
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
            let mut this = world.entry::<T>(handle).unwrap();
            this.trigger(&event);
        }));
    }
}
impl WorldCellElement<'_, dyn Element> {
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(&mut self, action: impl FnMut(&E, &WorldCell) + 'static) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry_dyn(handle).unwrap();
            this.observe(action);
        }));
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
            let mut this = world.entry_dyn(handle).unwrap();
            this.trigger(&event);
        }));
    }
}

// Internal Element #0
#[expect(clippy::type_complexity)]
struct Observers(HashMap<ElementHandle, Vec<Box<dyn FnMut(&dyn Any, &WorldCell)>>>);
impl Element for Observers {}

// Internal Element #1
#[derive(Default)]
#[expect(clippy::type_complexity)]
struct Queue(Vec<Box<dyn FnOnce(&mut World)>>);
impl Element for Queue {}

// World Events
pub struct ElementInserted(pub ElementHandle);
pub struct ElementRemoved(pub ElementHandle);
