use hashbrown::HashMap;

use crate::{
    interface::{Interface, Wireframe},
    lnwin::PointerEvent,
    measures::Rectangle,
    tools::pointer::{Pointer, PointerHit},
    world::{Destroy, Element, ElementHandle, InsertElement, WorldCell, WorldCellEntry},
};

pub struct NodeLinks {
    record: HashMap<ElementHandle, Link>,
}

struct Link {
    from: ElementHandle,
    to: ElementHandle,
    from_ob: ElementHandle,
    to_ob: ElementHandle,
}

impl NodeLinks {
    pub fn build_link(&mut self, from: ElementHandle, to: ElementHandle, world: &WorldCell) {
        if let (Some(from_entry), Some(to_entry)) = (world.entry(from), world.entry(to)) {
            let from_ob = from_entry.observe(move |Destroy, world| {
                let mut links = world.single_fetch_mut::<NodeLinks>().unwrap();
                links.remove_link(to, &world);
            });

            let to_ob = to_entry.observe(move |Destroy, world| {
                let mut links = world.single_fetch_mut::<NodeLinks>().unwrap();
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

impl Element for NodeLinks {}
impl InsertElement for NodeLinks {}

pub struct NodeTool {
    from_frame: Wireframe,
    to_frame: Wireframe,

    from_element: Option<ElementHandle>,
    to_element: Option<ElementHandle>,
}

impl NodeTool {
    pub fn new(interface: &mut Interface) -> NodeTool {
        NodeTool {
            from_frame: Wireframe::new(Rectangle::new(0, 0, 0, 0), [1.0, 0.0, 0.0, 1.0], interface),
            to_frame: Wireframe::new(Rectangle::new(0, 0, 0, 0), [0.0, 1.0, 0.0, 1.0], interface),
            from_element: None,
            to_element: None,
        }
    }
}

impl Element for NodeTool {}
impl InsertElement for NodeTool {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        if entry.single::<NodeLinks>().is_none() {
            entry.insert(NodeLinks {
                record: HashMap::new(),
            });
        }

        entry.observe(|PointerHit(event), entry| {
            let this = &mut *entry.fetch_mut().unwrap();

            match (event, this.from_element, this.to_element) {
                (PointerEvent::Pressed(point), _, _) => {
                    let pointer = entry.single_fetch::<Pointer>().unwrap();
                    this.from_element = pointer.intersect(&entry, *point);
                    if let Some(from) = this.from_element
                        && let Some(rectangle) = entry.get::<Rectangle>(from)
                    {
                        this.from_frame.set_visible(true);
                        this.from_frame.set_rect(rectangle);
                    }
                }
                (PointerEvent::Moved(point), Some(_), _) => {
                    let pointer = entry.single_fetch::<Pointer>().unwrap();
                    this.to_element = pointer.intersect(&entry, *point);
                    if let Some(to) = this.to_element
                        && let Some(rectangle) = entry.get::<Rectangle>(to)
                    {
                        this.to_frame.set_visible(true);
                        this.to_frame.set_rect(rectangle);
                    } else {
                        this.to_frame.set_visible(false);
                    }
                }
                (PointerEvent::Released(_), Some(from), Some(to)) => {
                    let mut links = entry.single_fetch_mut::<NodeLinks>().unwrap();
                    links.build_link(from, to, &entry);

                    this.from_element = None;
                    this.to_element = None;
                    this.from_frame.set_visible(false);
                    this.to_frame.set_visible(false);
                }
                (PointerEvent::Released(_), _, _) => {
                    this.from_element = None;
                    this.to_element = None;
                    this.from_frame.set_visible(false);
                    this.to_frame.set_visible(false);
                }
                _ => {}
            }
        });
    }
}
