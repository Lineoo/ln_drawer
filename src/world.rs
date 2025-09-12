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
    observers: HashMap<ElementHandle, Vec<Box<dyn FnMut(&dyn Any, &mut World)>>>,
}
impl World {
    pub fn insert(&mut self, element: impl Element + 'static) -> ElementHandle {
        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        ElementHandle(self.curr_idx.0 - 1)
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
            world: self,
            occupied: Mutex::new(HashMap::new()),
        }
    }

    pub fn entry<T: Element>(&mut self, handle: ElementHandle) -> WorldElement<'_, T> {
        WorldElement {
            world: self,
            handle,
            _marker: PhantomData,
        }
    }

    pub fn entry_dyn(&mut self, handle: ElementHandle) -> WorldElement<'_, dyn Element> {
        WorldElement {
            world: self,
            handle,
            _marker: PhantomData,
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

// Center of multiple accesses in world
pub struct WorldCell<'world> {
    world: &'world mut World,
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

    pub fn fetch_mut_dyn(&mut self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
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

    pub fn try_fetch<T: Element>(&self, handle: ElementHandle) -> Option<Ref<'_, T>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt < 0 {
            return None;
        }

        *cnt += 1;
        let element = self.world.elements.get(&handle)?.downcast_ref()?;

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
        let element = self.world.elements.get(&handle)?.as_ref();

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
        let element = self.world.elements.get(&handle)?.downcast_ref()?;

        Some(RefMut {
            ptr: element as *const T as *mut T,
            world: self,
            handle,
        })
    }

    pub fn try_fetch_mut_dyn(&mut self, handle: ElementHandle) -> Option<RefMut<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        let cnt = occupied.entry(handle).or_default();
        if *cnt != 0 {
            return None;
        }

        *cnt -= 1;
        let element = self.world.elements.get(&handle)?.as_ref();

        Some(RefMut {
            ptr: element as *const dyn Element as *mut dyn Element,
            world: self,
            handle,
        })
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
impl<T: Element> WorldElement<'_, T> {
    pub fn observe<E: 'static>(&mut self, mut action: impl FnMut(&E, &mut World) + 'static) {
        let observers = self.world.observers.entry(self.handle).or_default();
        observers.push(Box::new(move |event, world| {
            if let Some(event) = event.downcast_ref::<E>() {
                action(event, world);
            }
        }));
    }

    pub fn trigger<E: 'static>(&mut self, event: E) {
        if let Some(mut observers) = self.world.observers.remove(&self.handle) {
            for observer in &mut observers {
                observer(&event, self.world);
            }

            // If new observers are added during the scope, move it
            if let Some(changed) = self.world.observers.get_mut(&self.handle) {
                observers.append(changed);
            }

            self.world.observers.insert(self.handle, observers);
        }
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
