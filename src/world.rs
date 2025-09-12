use std::{
    any::Any,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use hashbrown::{DefaultHashBuilder, HashMap, HashSet};
use indexmap::IndexMap;
use parking_lot::Mutex;

use crate::elements::Element;

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementHandle(usize);

#[derive(Default)]
pub struct World {
    curr_idx: ElementHandle,
    elements: IndexMap<ElementHandle, Box<dyn Element>, DefaultHashBuilder>,
    observers: HashMap<ElementHandle, Vec<Box<dyn FnMut(&dyn Any, &mut World)>>>,

    // FIXME: in move
    occupied: Mutex<HashSet<ElementHandle>>,
}
impl World {
    pub fn insert(&mut self, element: impl Element + 'static) -> ElementHandle {
        self.elements.insert(self.curr_idx, Box::new(element));
        self.curr_idx.0 += 1;
        self.elements
            .sort_by(|_, c1, _, c2| c2.z_index().cmp(&c1.z_index()));
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

    // TODO not panic pls
    // TODO move fetch codes to the guard
    // FIXME immutable reference should be unaccessible while cell is fetched (New: WorldMutex)

    pub fn fetch_cell<T: Element>(&self, handle: ElementHandle) -> Option<WorldCell<'_, T>> {
        let mut occupied = self.occupied.lock();

        assert!(!occupied.contains(&handle), "{handle:?} occupied");
        let element = self.elements.get(&handle)?.downcast_ref()? as *const T;
        occupied.insert(handle);

        Some(WorldCell {
            // Cast safety: only one access is allowed
            ptr: element as *mut T,
            world: self,
            idx: handle,
        })
    }

    pub fn fetch_cell_dyn(&self, handle: ElementHandle) -> Option<WorldCell<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        assert!(!occupied.contains(&handle), "{handle:?} occupied");
        let element = self.elements.get(&handle)?.as_ref() as *const dyn Element;
        occupied.insert(handle);

        Some(WorldCell {
            // Cast safety: only one access is allowed
            ptr: element as *mut dyn Element,
            world: self,
            idx: handle,
        })
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

    pub fn intersect(&self, x: i32, y: i32) -> Option<ElementHandle> {
        for (idx, element) in &self.elements {
            let border = element.get_border();

            // Is in border
            if (x >= border[0] && x < border[2]) && (y >= border[1] && y < border[3]) {
                return Some(*idx);
            }
        }
        None
    }

    pub fn intersect_with<T: Element>(&self, x: i32, y: i32) -> Option<ElementHandle> {
        for (idx, element) in &self.elements {
            let border = element.get_border();

            // Is in border
            if (x >= border[0] && x < border[2])
                && (y >= border[1] && y < border[3])
                && element.is::<T>()
            {
                return Some(*idx);
            }
        }
        None
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

/// A world's limitedly mutable element reference.
pub struct WorldCell<'world, T: ?Sized> {
    ptr: *mut T,
    world: &'world World,
    idx: ElementHandle,
}
impl<T: ?Sized> Deref for WorldCell<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_ref().unwrap() }
    }
}
impl<T: ?Sized> DerefMut for WorldCell<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: guaranteed by World's cell_occupied
        unsafe { self.ptr.as_mut().unwrap() }
    }
}
impl<T: ?Sized> Drop for WorldCell<'_, T> {
    fn drop(&mut self) {
        let mut occupied = self.world.occupied.lock();
        occupied.remove(&self.idx);
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
