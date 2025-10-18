use std::marker::PhantomData;

use hashbrown::HashMap;

use crate::world::{Destroy, Element, ElementHandle, InsertElement, WorldCell};

pub struct NodeLinks<P> {
    record: HashMap<ElementHandle, Link>,
    _marker: PhantomData<P>,
}

struct Link {
    from: ElementHandle,
    to: ElementHandle,
    from_ob: ElementHandle,
    to_ob: ElementHandle,
}

impl<P: 'static> NodeLinks<P> {
    pub fn build_link(&mut self, from: ElementHandle, to: ElementHandle, world: &WorldCell) {
        if let (Some(from_entry), Some(to_entry)) = (world.entry(from), world.entry(to)) {
            let from_ob = from_entry.observe(move |Destroy, world| {
                let mut links = world.single_fetch_mut::<NodeLinks<P>>().unwrap();
                links.remove_link(to, &world);
            });

            let to_ob = to_entry.observe(move |Destroy, world| {
                let mut links = world.single_fetch_mut::<NodeLinks<P>>().unwrap();
                links.remove_link(to, &world);
            });

            let prev = self.record.insert(
                to,
                Link {
                    from,
                    to,
                    from_ob,
                    to_ob,
                },
            );

            if let Some(prev) = prev {
                world.remove(prev.from_ob);
                world.remove(prev.to_ob);
            }
        } else {
            log::error!("either {from} or {to} does not exist when attempting to link them");
        }
    }

    pub fn remove_link(&mut self, to: ElementHandle, world: &WorldCell) {
        if let Some(link) = self.record.remove(&to) {
            world.remove(link.from_ob);
            world.remove(link.to_ob);
        }
    }

    pub fn get_link(&self, to: ElementHandle) -> Option<ElementHandle> {
        self.record.get(&to).map(|link| link.from)
    }
}

impl<P: 'static> Element for NodeLinks<P> {}
impl<P: 'static> InsertElement for NodeLinks<P> {}
