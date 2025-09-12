use std::ops::{Deref, DerefMut};

use hashbrown::{DefaultHashBuilder, HashSet};
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

    pub fn fetch<T: Element + 'static>(&self, element_idx: ElementHandle) -> Option<&T> {
        self.elements
            .get(&element_idx)
            .and_then(|element| element.downcast_ref())
    }

    pub fn fetch_dyn(&self, element_idx: ElementHandle) -> Option<&dyn Element> {
        self.elements
            .get(&element_idx)
            .map(|element| element.as_ref())
    }

    pub fn fetch_mut<T: Element + 'static>(
        &mut self,
        element_idx: ElementHandle,
    ) -> Option<&mut T> {
        self.elements
            .get_mut(&element_idx)
            .and_then(|element| element.downcast_mut())
    }

    pub fn fetch_mut_dyn(&mut self, element_idx: ElementHandle) -> Option<&mut dyn Element> {
        self.elements
            .get_mut(&element_idx)
            .map(|element| element.as_mut())
    }

    // TODO not panic pls
    // TODO move fetch codes to the guard

    pub fn fetch_cell<T: Element + 'static>(
        &self,
        element_idx: ElementHandle,
    ) -> Option<WorldCell<'_, T>> {
        let mut occupied = self.occupied.lock();

        assert!(!occupied.contains(&element_idx), "{element_idx:?} occupied");
        let element = self.elements.get(&element_idx)?.downcast_ref()? as *const T;
        occupied.insert(element_idx);

        Some(WorldCell {
            // Cast safety: only one access is allowed
            ptr: element as *mut T,
            world: self,
            idx: element_idx,
        })
    }

    pub fn fetch_cell_dyn(&self, element_idx: ElementHandle) -> Option<WorldCell<'_, dyn Element>> {
        let mut occupied = self.occupied.lock();

        assert!(!occupied.contains(&element_idx), "{element_idx:?} occupied");
        let element = self.elements.get(&element_idx)?.as_ref() as *const dyn Element;
        occupied.insert(element_idx);

        Some(WorldCell {
            // Cast safety: only one access is allowed
            ptr: element as *mut dyn Element,
            world: self,
            idx: element_idx,
        })
    }

    // TODO Singleton-optimization

    /// Return `Some` if there is ONLY one element of target type.
    pub fn single<T: Element + 'static>(&self) -> Option<&T> {
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
    pub fn single_mut<T: Element + 'static>(&mut self) -> Option<&mut T> {
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
