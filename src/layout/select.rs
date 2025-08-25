use crate::{
    interface::{Interface, Wireframe},
    layout::world::{ElementHandle, World},
};

/// The main component for selection.
pub struct Selector {
    cursor: [i32; 2],

    hover_wireframe: Wireframe,
    selection_wireframe: Wireframe,

    selected_element: Option<ElementHandle>,
}
impl Selector {
    pub fn new(interface: &mut Interface) -> Selector {
        Selector {
            cursor: [0, 0],
            hover_wireframe: interface.create_wireframe([0, 0, 0, 0], [0.8, 0.2, 0.2, 1.0]),
            selection_wireframe: interface.create_wireframe([0, 0, 0, 0], [1.0, 0.0, 0.0, 1.0]),
            selected_element: None,
        }
    }
    pub fn cursor_position(&mut self, point: [i32; 2], world: &World) {
        self.cursor = point;
        let element = world.intersect(point[0], point[1]);
        if let Some(element) = element {
            let element = world.fetch_dyn(element).unwrap();
            let border = element.border();
            self.hover_wireframe.set_visible(true);
            self.hover_wireframe.set_rect(border);
        } else {
            self.hover_wireframe.set_visible(false);
        }
    }

    pub fn cursor_click(&mut self, world: &mut World) {
        let element = world.intersect(self.cursor[0], self.cursor[1]);
        self.selected_element = element;
        if let Some(element) = element {
            let element = world.fetch_mut_dyn(element).unwrap();
            let border = element.border();
            self.selection_wireframe.set_visible(true);
            self.selection_wireframe.set_rect(border);
        } else {
            self.selection_wireframe.set_visible(false);
        }
    }
}
