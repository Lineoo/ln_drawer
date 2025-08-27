use hashbrown::HashMap;

use crate::elements::Element;

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementHandle(usize);

pub struct World {
    element_idx: ElementHandle,
    elements: HashMap<ElementHandle, Box<dyn Element>>,
}
impl World {
    pub fn new() -> World {
        World {
            element_idx: ElementHandle(0),
            elements: HashMap::new(),
        }
    }

    pub fn insert(&mut self, element: impl Element + 'static) -> ElementHandle {
        self.elements.insert(self.element_idx, Box::new(element));
        self.element_idx.0 += 1;
        ElementHandle(self.element_idx.0 - 1)
    }

    pub fn fetch<T: 'static>(&self, element_idx: ElementHandle) -> Option<&T> {
        self.elements
            .get(&element_idx)
            .and_then(|element| element.downcast_ref())
    }

    pub fn fetch_dyn(&self, element_idx: ElementHandle) -> Option<&dyn Element> {
        self.elements
            .get(&element_idx)
            .map(|element| element.as_ref())
    }

    pub fn fetch_mut<T: 'static>(&mut self, element_idx: ElementHandle) -> Option<&mut T> {
        self.elements
            .get_mut(&element_idx)
            .and_then(|element| element.downcast_mut())
    }

    pub fn fetch_mut_dyn(&mut self, element_idx: ElementHandle) -> Option<&mut dyn Element> {
        self.elements
            .get_mut(&element_idx)
            .map(|element| element.as_mut())
    }

    pub fn intersect(&self, x: i32, y: i32) -> Option<ElementHandle> {
        for (idx, element) in &self.elements {
            let border = element.get_border();

            // Is in border
            if (x > border[0] && x < border[2]) && (y > border[1] && y < border[3]) {
                return Some(*idx);
            }
        }
        None
    }

    pub fn intersect_with<T: Element>(&self, x: i32, y: i32) -> Option<ElementHandle> {
        for (idx, element) in &self.elements {
            let border = element.get_border();

            // Is in border
            if (x > border[0] && x < border[2])
                && (y > border[1] && y < border[3])
                && element.is::<T>()
            {
                return Some(*idx);
            }
        }
        None
    }

    pub fn foreach<T: Element>(&self) -> impl Iterator<Item = &T> {
        (self.elements.values()).filter_map(|element| element.downcast_ref::<T>())
    }
}
