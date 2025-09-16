use std::{
    any::{Any, TypeId},
    cell::RefCell,
    ops::{Deref, DerefMut},
};

use hashbrown::HashMap;
use smallvec::SmallVec;

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

        elements.insert(ElementHandle(0), Box::new(Observers::default()));
        singletons.insert(
            TypeId::of::<Observers>(),
            Singleton::Unique(ElementHandle(0)),
        );

        elements.insert(ElementHandle(1), Box::new(Queue::default()));
        singletons.insert(TypeId::of::<Queue>(), Singleton::Unique(ElementHandle(1)));

        elements.insert(ElementHandle(2), Box::new(Services::default()));
        singletons.insert(
            TypeId::of::<Services>(),
            Singleton::Unique(ElementHandle(2)),
        );

        World {
            curr_idx: ElementHandle(3),
            elements,
            singletons,
        }
    }
}
impl World {
    pub fn insert<T: Element + 'static>(&mut self, element: T) -> ElementHandle {
        let type_id = element.type_id();

        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        let handle = ElementHandle(self.curr_idx.0 - 1);

        // this-type service register
        let mut entry = self.entry(handle).unwrap();
        entry.register::<T>(|this| this.downcast_ref::<T>().unwrap());
        entry.register_mut::<T>(|this| this.downcast_mut::<T>().unwrap());
        entry.register::<dyn Element>(|this| this.downcast_ref::<T>().unwrap());
        entry.register_mut::<dyn Element>(|this| this.downcast_mut::<T>().unwrap());

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

        // when_inserted
        let cell = self.cell();
        let mut element = cell.fetch_mut_dyn(handle).unwrap();
        element.when_inserted(handle, &cell);
        drop(element);
        drop(cell);

        // ElementInserted
        self.trigger(&ElementInserted(handle));

        handle
    }

    pub fn remove(&mut self, handle: ElementHandle) -> Option<Box<dyn Element>> {
        let type_id = (**self.elements.get(&handle)?).type_id();

        // ElementRemoved
        self.trigger(&ElementRemoved(handle));

        // when_removed
        let cell = self.cell();
        let mut element = cell.fetch_mut_dyn(handle).unwrap();
        element.when_removed(handle, &cell);
        drop(element);
        drop(cell);

        // remove related services
        for services_typed in &mut self.single_mut::<Services>().unwrap().0 {
            services_typed.1.remove(&handle);
        }

        // singleton cache
        let singleton = self.singletons.get_mut(&type_id).unwrap();
        match singleton {
            Singleton::Unique(_) => {
                self.singletons.remove(&type_id);
            }
            Singleton::Multiple(cnt) => {
                // We don't actually consider the situation that multiple elements being remove until
                // one is left. In such case, even though there technically is only *one* element, which
                // should be singleton, but mostly it won't be used as a singleton, and use loops to cache
                // it is basically a waste. So we won't implement it.
                *cnt -= 1;
                if *cnt == 0 {
                    self.singletons.remove(&type_id);
                }
            }
        }

        // clean invalid observers
        let mut attached_observers = Vec::with_capacity(4);
        for observers_typed in &self.single::<Observers>().unwrap().0 {
            if let Some(observers_typed_element) = (observers_typed.1).get(&handle) {
                for observer in observers_typed_element {
                    attached_observers.push(*observer);
                }
            }
        }

        for observer in attached_observers {
            self.remove(observer);
        }

        self.elements.remove(&handle)
    }

    pub fn contains(&self, handle: ElementHandle) -> bool {
        self.elements.contains_key(&handle)
    }

    pub fn contains_type<T: ?Sized + 'static>(&self, handle: ElementHandle) -> bool {
        let services = self.single::<Services>().unwrap();
        if let Some(services_typed) = services.0.get(&TypeId::of::<ServicesTyped<T>>()) {
            let services_typed = services_typed.downcast_ref::<ServicesTyped<T>>().unwrap();
            services_typed.0.contains_key(&handle)
        } else {
            false
        }
    }

    pub fn contains_raw<T: Element>(&self, handle: ElementHandle) -> bool {
        self.elements
            .get(&handle)
            .is_some_and(|element| element.is::<T>())
    }

    pub fn fetch<T: ?Sized + 'static>(&self, handle: ElementHandle) -> Option<&T> {
        let element = self.elements.get(&handle)?.as_ref();

        let services = self.single::<Services>().unwrap();
        let services_typed = services.0.get(&TypeId::of::<ServicesTyped<T>>())?;
        let services_typed = services_typed.downcast_ref::<ServicesTyped<T>>().unwrap();
        let service = services_typed.0.get(&handle)?;
        Some(service(element))
    }

    pub fn fetch_raw<T: Element>(&self, handle: ElementHandle) -> Option<&T> {
        self.elements
            .get(&handle)
            .and_then(|element| element.downcast_ref())
    }

    pub fn fetch_dyn(&self, handle: ElementHandle) -> Option<&dyn Element> {
        self.elements.get(&handle).map(|element| element.as_ref())
    }

    pub fn fetch_mut<T: ?Sized + 'static>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        let services = self.single::<Services>().unwrap() as *const Services;
        let element = self.elements.get_mut(&handle)?.as_mut();
        let element_ptr = element as *mut dyn Element;

        let services = unsafe { services.as_ref().unwrap() };
        let services_typed = services.0.get(&TypeId::of::<ServicesTypedMut<T>>())?;
        let services_typed = services_typed
            .downcast_ref::<ServicesTypedMut<T>>()
            .unwrap();
        let service = services_typed.0.get(&handle)?;
        Some(service(unsafe { element_ptr.as_mut().unwrap() }))
    }

    pub fn fetch_mut_raw<T: Element>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        self.elements
            .get_mut(&handle)
            .and_then(|element| element.downcast_mut())
    }

    pub fn fetch_mut_dyn(&mut self, handle: ElementHandle) -> Option<&mut dyn Element> {
        self.elements
            .get_mut(&handle)
            .map(|element| element.as_mut())
    }

    pub fn entry(&mut self, handle: ElementHandle) -> Option<WorldEntry<'_>> {
        if !self.elements.contains_key(&handle) {
            return None;
        }

        Some(WorldEntry {
            world: self,
            handle,
        })
    }

    pub fn foreach<T: ?Sized + 'static>(&self, mut action: impl FnMut(&T)) {
        let services = self.single::<Services>().unwrap();
        if let Some(services_typed) = services.0.get(&TypeId::of::<ServicesTyped<T>>()) {
            let services_typed = services_typed.downcast_ref::<ServicesTyped<T>>().unwrap();
            services_typed.0.iter().for_each(|(handle, converter)| {
                let service = converter(self.elements.get(handle).unwrap().as_ref());
                action(service);
            });
        }
    }

    pub fn foreach_mut<T: ?Sized + 'static>(&mut self, mut action: impl FnMut(&mut T)) {
        let services = self.single::<Services>().unwrap() as *const Services;
        let services = unsafe { services.as_ref().unwrap() };
        if let Some(services_typed) = services.0.get(&TypeId::of::<ServicesTypedMut<T>>()) {
            let services_typed = services_typed
                .downcast_ref::<ServicesTypedMut<T>>()
                .unwrap();
            services_typed.0.iter().for_each(|(handle, converter)| {
                let service = converter(self.elements.get_mut(handle).unwrap().as_mut());
                action(service);
            });
        }
    }

    // Singleton

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
            occupied: RefCell::new(HashMap::new()),
        }
    }

    /// Global trigger. Will trigger every element listening to this event.
    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let cell = self.cell();
        let observers = cell.single_mut::<Observers>().unwrap();
        if let Some(observers_typed) = observers.0.get(&TypeId::of::<E>()) {
            for observer in observers_typed.values().flatten() {
                let mut observer = cell.fetch_mut::<Observer>(*observer).unwrap();
                (observer.0)(event, &cell);
            }
        }
    }
}

/// A full mutable world reference with specific element selected.
pub struct WorldEntry<'world> {
    world: &'world mut World,
    handle: ElementHandle,
}
impl WorldEntry<'_> {
    pub fn observe<E: 'static>(&mut self, mut action: impl FnMut(&E, &WorldCell) + 'static) {
        let handle = self.world.insert(Observer(Box::new(move |event, world| {
            let event = event.downcast_ref::<E>().unwrap();
            action(event, world);
        })));
        let observers = self.world.single_mut::<Observers>().unwrap();
        let observers_typed = (observers.0).entry(TypeId::of::<E>()).or_default();
        let observers_typed_element = observers_typed.entry(self.handle).or_default();
        observers_typed_element.push(handle);
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        let cell = self.world.cell();
        let observers = cell.single::<Observers>().unwrap();
        if let Some(observers_typed) = observers.0.get(&TypeId::of::<E>())
            && let Some(observers_typed_element) = observers_typed.get(&self.handle)
        {
            for observer in observers_typed_element {
                let mut observer = cell.fetch_mut::<Observer>(*observer).unwrap();
                (observer.0)(event, &cell);
            }
        }
    }

    pub fn register<U: ?Sized + 'static>(
        &mut self,
        service: impl Fn(&dyn Element) -> &U + 'static,
    ) {
        let cell = self.world.cell();
        let mut services = cell.single_mut::<Services>().unwrap();
        let services_typed = (services.0)
            .entry(TypeId::of::<ServicesTyped<U>>())
            .or_insert_with(|| Box::new(ServicesTyped::<U>(HashMap::new())))
            .downcast_mut::<ServicesTyped<U>>()
            .unwrap();

        let popback = services_typed.0.insert(self.handle, Box::new(service));
        if popback.is_some() {
            log::error!(
                "duplicated service of type \"{}\" is registered on {:?}",
                std::any::type_name::<U>(),
                self.handle
            );
        }
    }

    pub fn register_mut<U: ?Sized + 'static>(
        &mut self,
        service: impl Fn(&mut dyn Element) -> &mut U + 'static,
    ) {
        let cell = self.world.cell();
        let mut services = cell.single_mut::<Services>().unwrap();
        let services_typed = (services.0)
            .entry(TypeId::of::<ServicesTypedMut<U>>())
            .or_insert_with(|| Box::new(ServicesTypedMut::<U>(HashMap::new())))
            .downcast_mut::<ServicesTypedMut<U>>()
            .unwrap();

        let popback = services_typed.0.insert(self.handle, Box::new(service));
        if popback.is_some() {
            log::error!(
                "duplicated service of type \"{}\" is registered on {:?}",
                std::any::type_name::<U>(),
                self.handle
            );
        }
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, other: ElementHandle) {
        let handle = self.handle;
        self.observe::<ElementRemoved>(move |event, world| {
            if event.0 == other {
                // TODO wait until cell-mode removal is implemented
                let mut queue = world.single_mut::<Queue>().unwrap();
                queue.0.push(Box::new(move |world| {
                    world.remove(handle);
                }));
            }
        });
    }
}

// Center of multiple accesses in world, which also prevents constructional changes
pub struct WorldCell<'world> {
    world: &'world mut World,
    occupied: RefCell<HashMap<ElementHandle, isize>>,
}
impl Drop for WorldCell<'_> {
    fn drop(&mut self) {
        let queue = self.world.single_mut::<Queue>().unwrap();
        let mut buf = Vec::new();
        buf.append(&mut queue.0);

        for cmd in buf {
            cmd(self.world);
        }
    }
}
impl WorldCell<'_> {
    pub fn fetch<T: ?Sized + 'static>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            panic!("{handle:?} is mutably borrowed");
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle)?.as_ref();

        let services = self.world.single::<Services>().unwrap();
        let services_typed = services.0.get(&TypeId::of::<ServicesTyped<T>>())?;
        let services_typed = services_typed.downcast_ref::<ServicesTyped<T>>().unwrap();
        let service = services_typed.0.get(&handle)?;
        let ptr = service(element) as *const T;

        Some(Ref {
            ptr,
            world: self,
            handle,
        })
    }

    pub fn fetch_raw<T: Element>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
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

    pub fn fetch_dyn(&self, handle: ElementHandle) -> Option<Ref<'_, dyn Element>> {
        let mut occupied = self.occupied.borrow_mut();

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

    pub fn fetch_mut<T: ?Sized + 'static>(&self, handle: ElementHandle) -> Option<RefMut<'_, T>> {
        let mut occupied = self.occupied.borrow_mut();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            panic!("{handle:?} is borrowed");
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle)?.as_ref();

        // SAFETY: The services set is immutable during cell span
        let services = self.world.single::<Services>().unwrap();
        let services_typed = services.0.get(&TypeId::of::<ServicesTypedMut<T>>())?;
        let services_typed = services_typed
            .downcast_ref::<ServicesTypedMut<T>>()
            .unwrap();
        let service = services_typed.0.get(&handle)?;
        let element = element as *const dyn Element as *mut dyn Element;
        let element = unsafe { element.as_mut().unwrap() };
        let ptr = service(element) as *mut T;

        Some(RefMut {
            ptr,
            world: self,
            handle,
        })
    }

    pub fn fetch_mut_raw<T: Element>(&self, handle: ElementHandle) -> Option<RefMut<'_, T>> {
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

    pub fn fetch_mut_dyn(&self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
        let mut occupied = self.occupied.borrow_mut();

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

    pub fn entry(&self, handle: ElementHandle) -> Option<WorldCellEntry<'_>> {
        if !self.world.elements.contains_key(&handle) {
            return None;
        }

        Some(WorldCellEntry {
            world: self,
            handle,
        })
    }

    pub fn foreach<T: ?Sized + 'static>(&self, mut action: impl FnMut(&T)) {
        let services = self.world.single::<Services>().unwrap();
        if let Some(services_typed) = services.0.get(&TypeId::of::<ServicesTyped<T>>()) {
            let services_typed = services_typed.downcast_ref::<ServicesTyped<T>>().unwrap();
            services_typed.0.iter().for_each(|(handle, converter)| {
                let occupied = self.occupied.borrow();
                if occupied.get(handle).is_some_and(|cnt| *cnt < 0) {
                    log::warn!("{handle:?} is mutably borrowed during `foreach`, skipped");
                    return;
                }

                let service = converter(self.world.elements.get(handle).unwrap().as_ref());
                action(service);
            });
        }
    }

    pub fn foreach_mut<T: ?Sized + 'static>(&self, mut action: impl FnMut(&mut T)) {
        let services = self.world.single::<Services>().unwrap();
        if let Some(services_typed) = services.0.get(&TypeId::of::<ServicesTypedMut<T>>()) {
            let services_typed = services_typed
                .downcast_ref::<ServicesTypedMut<T>>()
                .unwrap();
            services_typed.0.iter().for_each(|(handle, converter)| {
                let occupied = self.occupied.borrow();
                if occupied.get(handle).is_some_and(|cnt| *cnt != 0) {
                    log::warn!("{handle:?} is borrowed during `foreach_mut`, skipped");
                    return;
                }

                let element = self.world.elements.get(handle).unwrap().as_ref();
                let element = element as *const dyn Element as *mut dyn Element;
                let service = converter(unsafe { element.as_mut().unwrap() });
                action(service);
            });
        }
    }

    // Singleton

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<Ref<'_, T>> {
        if let Some(Singleton::Unique(handle)) = self.world.singletons.get(&TypeId::of::<T>()) {
            self.fetch_raw(*handle)
        } else {
            None
        }
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        if let Some(Singleton::Unique(handle)) = self.world.singletons.get(&TypeId::of::<T>()) {
            self.fetch_mut_raw(*handle)
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
        let mut occupied = self.world.occupied.borrow_mut();
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
        let mut occupied = self.world.occupied.borrow_mut();
        let cnt = occupied.get_mut(&self.handle).unwrap();
        *cnt += 1;
    }
}

/// A world cell reference with specific element selected. No borrowing effect.
pub struct WorldCellEntry<'world> {
    world: &'world WorldCell<'world>,
    handle: ElementHandle,
}
impl WorldCellEntry<'_> {
    /// This will be delayed until the cell is closed. So not all triggers in the cell scope would come into
    /// effect (by its adding order instead).
    pub fn observe<E: 'static>(&mut self, action: impl FnMut(&E, &WorldCell) + 'static) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
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
            let mut this = world.entry(handle).unwrap();
            this.trigger(&event);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn register<U: ?Sized + 'static>(
        &mut self,
        service: impl Fn(&dyn Element) -> &U + 'static,
    ) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.register(service);
        }));
    }

    /// This will be delayed until the cell is closed.
    pub fn register_mut<U: ?Sized + 'static>(
        &mut self,
        service: impl Fn(&mut dyn Element) -> &mut U + 'static,
    ) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.register_mut(service);
        }));
    }

    /// Declare a dependency relationship. When the `other` Element is removed, this element
    /// will be removed as well. Useful for keeping handle valid.
    pub fn depend(&mut self, other: ElementHandle) {
        let handle = self.handle;
        let mut queue = self.world.single_mut::<Queue>().unwrap();
        queue.0.push(Box::new(move |world| {
            let mut this = world.entry(handle).unwrap();
            this.depend(other);
        }));
    }
}

// Internal Element #0
#[derive(Default)]
struct Observers(HashMap<TypeId, HashMap<ElementHandle, SmallVec<[ElementHandle; 1]>>>);
#[expect(clippy::type_complexity)]
struct Observer(Box<dyn FnMut(&dyn Any, &WorldCell)>);
impl Element for Observers {}
impl Element for Observer {}

// Internal Element #1
#[derive(Default)]
#[expect(clippy::type_complexity)]
struct Queue(Vec<Box<dyn FnOnce(&mut World)>>);
impl Element for Queue {}

// Internal Element #2
#[derive(Default)]
struct Services(HashMap<TypeId, Box<dyn ServicesPart>>);
struct ServicesTyped<U: ?Sized>(HashMap<ElementHandle, Service<U>>);
struct ServicesTypedMut<U: ?Sized>(HashMap<ElementHandle, ServiceMut<U>>);
type Service<U> = Box<dyn Fn(&dyn Element) -> &U>;
type ServiceMut<U> = Box<dyn Fn(&mut dyn Element) -> &mut U>;
impl Element for Services {}

trait ServicesPart: Any {
    fn remove(&mut self, handle: &ElementHandle);
}
impl<U: ?Sized + 'static> ServicesPart for ServicesTyped<U> {
    fn remove(&mut self, handle: &ElementHandle) {
        self.0.remove(handle);
    }
}
impl<U: ?Sized + 'static> ServicesPart for ServicesTypedMut<U> {
    fn remove(&mut self, handle: &ElementHandle) {
        self.0.remove(handle);
    }
}
impl dyn ServicesPart {
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}

// World Events
pub struct ElementInserted(pub ElementHandle);
pub struct ElementRemoved(pub ElementHandle);

#[cfg(test)]
mod test {
    use crate::{
        elements::{Element, PositionedElement},
        world::{ElementHandle, World, WorldCell},
    };

    struct TestElement {
        position: [i32; 2],
    }
    impl PositionedElement for TestElement {
        fn get_position(&self) -> [i32; 2] {
            self.position
        }
        fn set_position(&mut self, position: [i32; 2]) {
            self.position = position;
        }
    }
    impl Element for TestElement {
        fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
            (world.entry(handle).unwrap())
                .register(|this| &this.downcast_ref::<TestElement>().unwrap().position);
            (world.entry(handle).unwrap())
                .register_mut(|this| &mut this.downcast_mut::<TestElement>().unwrap().position);
            (world.entry(handle).unwrap()).register::<dyn PositionedElement>(|this| {
                this.downcast_ref::<TestElement>().unwrap()
            });
            (world.entry(handle).unwrap()).register_mut::<dyn PositionedElement>(|this| {
                this.downcast_mut::<TestElement>().unwrap()
            });
        }
    }

    #[test]
    fn service() {
        let mut world = World::default();
        let handle = world.insert(TestElement {
            position: [42, 123],
        });
        let position = world.fetch_mut::<[i32; 2]>(handle).unwrap();
        assert_eq!(position, &[42, 123]);
        position[1] = 321;
    }

    #[test]
    fn unregistered_service() {
        let mut world = World::default();
        let handle = world.insert(TestElement {
            position: [42, 123],
        });
        assert_eq!(world.fetch::<i32>(handle), None);
    }

    #[test]
    fn dynamic_service() {
        let mut world = World::default();
        let handle = world.insert(TestElement {
            position: [42, 123],
        });
        let position = world.fetch::<dyn PositionedElement>(handle).unwrap();
        assert_eq!(position.get_position(), [42, 123]);
    }
}
