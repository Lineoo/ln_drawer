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

    drag_start: Option<[i32; 2]>,
    drag_element_orig: Option<[i32; 2]>,
}
impl Selector {
    pub fn new(interface: &mut Interface) -> Selector {
        let hover_wireframe = interface.create_wireframe([0, 0, 0, 0], [0.8, 0.2, 0.2, 1.0]);
        let selection_wireframe = interface.create_wireframe([0, 0, 0, 0], [1.0, 0.0, 0.0, 1.0]);

        hover_wireframe.set_z_order(100);
        selection_wireframe.set_z_order(100);

        Selector {
            cursor: [0, 0],
            hover_wireframe,
            selection_wireframe,
            selected_element: None,
            drag_start: None,
            drag_element_orig: None,
        }
    }
    
    pub fn cursor_position(&mut self, point: [i32; 2], world: &mut World) {
        self.cursor = point;
        if let Some(selected) = self.selected_element
            && let Some(start) = self.drag_start
            && let Some(elm_orig) = self.drag_element_orig
        {
            let dx = self.cursor[0] - start[0];
            let dy = self.cursor[1] - start[1];

            let selected = world.fetch_mut_dyn(selected).unwrap();
            selected.set_position([elm_orig[0] + dx, elm_orig[1] + dy]);

            let border = selected.get_border();
            self.selection_wireframe.set_visible(true);
            self.selection_wireframe.set_rect(border);
        }

        let element = world.intersect(point[0], point[1]);
        if let Some(element) = element {
            let element = world.fetch_dyn(element).unwrap();
            let border = element.get_border();
            self.hover_wireframe.set_visible(true);
            self.hover_wireframe.set_rect(border);
        } else {
            self.hover_wireframe.set_visible(false);
        }
    }

    pub fn cursor_pressed(&mut self, world: &mut World) {
        let element = world.intersect(self.cursor[0], self.cursor[1]);
        self.selected_element = element;
        if let Some(element) = element {
            let element = world.fetch_mut_dyn(element).unwrap();
            let border = element.get_border();
            self.selection_wireframe.set_visible(true);
            self.selection_wireframe.set_rect(border);

            self.drag_start = Some(self.cursor);
            self.drag_element_orig = Some(element.get_position());
        } else {
            self.selection_wireframe.set_visible(false);
        }
    }

    pub fn cursor_released(&mut self) {
        self.drag_start = None;
        self.drag_element_orig = None;
    }
    
    pub fn stop(&mut self) {
        self.selected_element = None;
        self.hover_wireframe.set_visible(false);
        self.selection_wireframe.set_visible(false);
    }
}
