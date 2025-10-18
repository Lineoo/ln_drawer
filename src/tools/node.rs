use std::marker::PhantomData;

use hashbrown::HashMap;

use crate::world::{Destroy, Element, ElementHandle, WorldCell};

pub struct NodeLinks<P> {
    invert: HashMap<ElementHandle, ElementHandle>,
    _marker: PhantomData<P>,
}

impl<P: 'static> NodeLinks<P> {
    pub fn build_link(&mut self, from: ElementHandle, to: ElementHandle, world: &WorldCell) {
        if let (Some(from_entry), Some(to_entry)) = (world.entry(from), world.entry(to)) {
            self.invert.insert(to, from);

            let from_ob = from_entry.observe(move |Destroy, entry| {
                let mut links = entry.single_fetch_mut::<NodeLinks<P>>().unwrap();
                links.invert.remove(&to);
            });

            let to_ob = to_entry.observe(move |Destroy, entry| {
                let mut links = entry.single_fetch_mut::<NodeLinks<P>>().unwrap();
                links.invert.remove(&to);
            });

            world.entry(to_ob).unwrap().depend(from_ob);
            world.entry(from_ob).unwrap().depend(to_ob);
        } else {
            log::error!("{from} or {to} does not exist when attempting to link them");
        }
    }
}

impl<P: 'static> Element for NodeLinks<P> {}
