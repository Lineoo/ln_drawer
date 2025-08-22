use std::any::Any;

use hashbrown::HashMap;

use crate::elements::Element;

/// Represent an element in the [`World`]. It's an handle so manual validation is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementIdx(usize);

pub struct World {
    element_idx: ElementIdx,
    elements: HashMap<ElementIdx, Box<dyn Element>>,
}
impl World {
    pub fn new() -> World {
        World {
            element_idx: ElementIdx(0),
            elements: HashMap::new(),
        }
    }

    pub fn insert(&mut self, element: impl Element + 'static) -> ElementIdx {
        self.elements.insert(self.element_idx, Box::new(element));
        self.element_idx.0 += 1;
        ElementIdx(self.element_idx.0 - 1)
    }

    pub fn fetch<T: 'static>(&mut self, element_idx: ElementIdx) -> Option<&mut T> {
        self.elements
            .get_mut(&element_idx)
            .and_then(|element| (element.as_mut() as &mut dyn Any).downcast_mut())
    }

    pub fn fetch_dyn(&mut self, element_idx: ElementIdx) -> Option<&mut dyn Element> {
        self.elements
            .get_mut(&element_idx)
            .map(|element| element.as_mut())
    }

    pub fn intersect(&self, x: i32, y: i32) -> Option<ElementIdx> {
        for (idx, element) in &self.elements {
            let border = element.border();

            // Is in border
            if (x > border[0] && x < border[2]) && (y > border[1] && y < border[3]) {
                return Some(*idx);
            }
        }
        None
    }
}
