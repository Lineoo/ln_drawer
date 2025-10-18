use crate::{
    interface::{Interface, StandardSquare},
    lnwin::PointerEvent,
    measures::{Delta, Rectangle, ZOrder},
    text::Text,
    tools::{
        node::NodeLinks,
        pointer::{PointerCollider, PointerEnter, PointerHit, PointerLeave},
    },
    world::{Element, InsertElement, WorldCellEntry},
};

type ButtonAction = Box<dyn FnMut(WorldCellEntry<ButtonRaw>)>;

/// Only contains raw button interaction logic. See [`Button`] if a complete button
/// including text and image is needed.
pub struct ButtonRaw {
    square: StandardSquare,
    action: ButtonAction,
}
impl Element for ButtonRaw {}
impl InsertElement for ButtonRaw {
    fn when_inserted(&mut self, entry: WorldCellEntry<Self>) {
        entry.observe::<PointerHit>(move |event, entry| {
            if let PointerHit(PointerEvent::Pressed(_)) = event {
                let mut this = entry.fetch_mut().unwrap();
                let mut temp: ButtonAction = Box::new(|_| ());
                std::mem::swap(&mut this.action, &mut temp);
                drop(this);

                temp(entry.clone());

                let mut this = entry.fetch_mut().unwrap();
                std::mem::swap(&mut this.action, &mut temp);
            }
        });

        entry.observe::<PointerEnter>(move |_event, entry| {
            let mut this = entry.fetch_mut().unwrap();
            this.square.set_visible(true);
        });
        entry.observe::<PointerLeave>(move |_event, entry| {
            let mut this = entry.fetch_mut().unwrap();
            this.square.set_visible(false);
        });

        entry.getter::<PointerCollider>(|this| PointerCollider {
            rect: this.square.get_rect(),
            z_order: this.square.get_z_order(),
        });

        entry.getter::<Rectangle>(|this| this.square.get_rect());
        entry.setter::<Rectangle>(|this, rect| this.square.set_rect(rect));
    }
}
impl ButtonRaw {
    pub fn new(
        rect: Rectangle,
        z_order: ZOrder,
        action: impl FnMut(WorldCellEntry<Self>) + 'static,
        interface: &mut Interface,
    ) -> ButtonRaw {
        ButtonRaw {
            square: StandardSquare::new(rect, z_order, false, interface),
            action: Box::new(action),
        }
    }

    pub fn shell(rect: Rectangle, z_order: ZOrder, interface: &mut Interface) -> ButtonRaw {
        ButtonRaw {
            square: StandardSquare::new(rect, z_order, false, interface),
            action: Box::new(|entry| {
                if let Some(links) = entry.single_fetch::<NodeLinks>()
                    && let Some(source) = links.get_link(entry.handle().untyped())
                    && let Some(text) = entry.get::<String>(source)
                {
                    match std::process::Command::new(text).output() {
                        Ok(output) => {
                            let curr_rect =
                                entry.get::<Rectangle>(entry.handle().untyped()).unwrap();
                            entry.insert(Text::new(
                                curr_rect.with_origin(curr_rect.origin + Delta::splat(40)),
                                String::from_utf8_lossy(&output.stdout).into(),
                                &mut entry.single_fetch_mut().unwrap(),
                                &mut entry.single_fetch_mut().unwrap(),
                            ));
                        }
                        Err(error) => {
                            let curr_rect =
                                entry.get::<Rectangle>(entry.handle().untyped()).unwrap();
                            entry.insert(Text::new(
                                curr_rect.with_origin(curr_rect.origin + Delta::splat(40)),
                                error.to_string(),
                                &mut entry.single_fetch_mut().unwrap(),
                                &mut entry.single_fetch_mut().unwrap(),
                            ));
                        }
                    }
                }
            }),
        }
    }
}
