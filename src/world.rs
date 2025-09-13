use std::{
    any::Any,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use hashbrown::HashMap;
use parking_lot::Mutex;

use crate::elements::Element;

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementHandle(usize);

#[derive(Default)]
#[expect(clippy::type_complexity)]
pub struct World {
    curr_idx: ElementHandle,
    elements: HashMap<ElementHandle, Box<dyn Element>>,
    observers: HashMap<ElementHandle, Vec<Box<dyn FnMut(&dyn Any, &mut WorldCell)>>>,
}
impl World {
    pub fn insert(&mut self, element: impl Element + 'static) -> ElementHandle {
        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        let handle = ElementHandle(self.curr_idx.0 - 1);

        // when_inserted
        let mut queue = WorldQueue::default();
        let element = self.fetch_mut_dyn(handle).unwrap();
        element.when_inserted(handle, &mut queue);
        queue.flush(self);

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

    pub fn cell(&mut self) -> WorldCell<'_> {
        WorldCell {
            elements: &mut self.elements,
            occupied: Mutex::new(HashMap::new()),
        }
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

    /// Global trigger. Will trigger every element listening to this event.
    pub fn trigger<E: 'static>(&mut self, event: &E) {
        // Manually construct to allow `observers` and `elements` can be separately mutable
        let mut cell = WorldCell {
            elements: &mut self.elements,
            occupied: Mutex::default(),
        };

        for observers in self.observers.values_mut() {
            for observer in observers {
                observer(event, &mut cell);
            }
        }
    }

    // TODO Singleton-optimization

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<&T> {
        let mut ret = None;
        for element in self.elements::<T>() {
            if ret.is_none() {
                ret.replace(element);
            } else {
                return None;
            }
        }
        ret
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&mut self) -> Option<&mut T> {
        let mut ret = None;
        for element in self.elements_mut::<T>() {
            if ret.is_none() {
                ret.replace(element);
            } else {
                return None;
            }
        }
        ret
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

// Center of multiple accesses in world, which also prevents constructional changes
pub struct WorldCell<'world> {
    elements: &'world mut HashMap<ElementHandle, Box<dyn Element>>,
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
        let element = self.elements.get(&handle)?.downcast_ref()?;

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
        let element = self.elements.get(&handle)?.as_ref();

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
        let element = self.elements.get(&handle)?.downcast_ref()?;

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
        let element = self.elements.get(&handle)?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
        })
    }

    pub fn try_fetch<T: Element>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            return None;
        }

        *cnt += 1;
        let element = self.elements.get(&handle)?.downcast_ref()?;

        Some(Ref {
            ptr: element as *const T,
            world: self,
            handle,
        })
    }

    pub fn try_fetch_dyn(&self, handle: ElementHandle) -> Option<Ref<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            return None;
        }

        *cnt += 1;
        let element = self.elements.get(&handle)?.as_ref();

        Some(Ref {
            ptr: element as *const dyn Element,
            world: self,
            handle,
        })
    }

    pub fn try_fetch_mut<T: Element>(&self, handle: ElementHandle) -> Option<RefMut<'_, T>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            return None;
        }

        *cnt -= 1;
        let element = self.elements.get(&handle)?.downcast_ref()?;

        Some(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
        })
    }

    pub fn try_fetch_mut_dyn(&self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            return None;
        }

        *cnt -= 1;
        let element = self.elements.get(&handle)?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
        })
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element>(&self) -> Option<Ref<'_, T>> {
        let mut ret = None;
        for (handle, element) in self.elements.iter() {
            let occupied = self.occupied.lock();
            if occupied.contains_key(handle) || !element.is::<T>() {
                continue;
            }

            if ret.is_none() {
                ret.replace(*handle);
            } else {
                return None;
            }
        }

        self.fetch(ret?)
    }

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single_mut<T: Element>(&self) -> Option<RefMut<'_, T>> {
        let mut ret = None;
        for (handle, element) in self.elements.iter() {
            let occupied = self.occupied.lock();
            if occupied.contains_key(handle) || !element.is::<T>() {
                continue;
            }

            if ret.is_none() {
                ret.replace(*handle);
            } else {
                return None;
            }
        }

        self.fetch_mut(ret?)
    }

    // Direct occupation skipping the lock

    pub fn occupy<T: Element>(&mut self, handle: ElementHandle) -> Option<&mut T> {
        self.elements.get_mut(&handle)?.downcast_mut()
    }

    pub fn occupy_dyn(&mut self, handle: ElementHandle) -> Option<&mut dyn Element> {
        self.elements.get_mut(&handle).map(|elm| elm.as_mut())
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
    pub fn observe<E: 'static>(&mut self, mut action: impl FnMut(&E, &mut WorldCell) + 'static) {
        let observers = self.world.observers.entry(self.handle).or_default();
        observers.push(Box::new(move |event, world| {
            if let Some(event) = event.downcast_ref::<E>() {
                action(event, world);
            }
        }));
    }

    pub fn trigger<E: 'static>(&mut self, event: &E) {
        // Manually construct to allow `observers` and `elements` can be separately mutable
        let mut cell = WorldCell {
            elements: &mut self.world.elements,
            occupied: Mutex::default(),
        };

        if let Some(observers) = self.world.observers.get_mut(&self.handle) {
            for observer in observers {
                observer(event, &mut cell);
            }
        }
    }
}

#[derive(Default)]
#[expect(clippy::type_complexity)]
pub struct WorldQueue {
    queue: Vec<Box<dyn FnOnce(&mut World)>>,
}
impl WorldQueue {
    pub fn queue(&mut self, ops: impl FnOnce(&mut World) + 'static) {
        self.queue.push(Box::new(ops));
    }

    fn flush(self, world: &mut World) {
        for cmd in self.queue {
            cmd(world);
        }
    }
}
