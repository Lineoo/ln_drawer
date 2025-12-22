use crate::{
    measures::{Position, Rectangle},
    render::wireframe::Wireframe,
    tools::pointer::PointerCollider,
    world::{Element, Handle, World},
};

pub struct Transform {
    pub rect: Rectangle,
    pub resizable: bool,
}

pub struct TransformUpdate;

impl Element for Transform {}

#[derive(Default)]
pub struct TransformTool {
    active: Option<Active>,
}

struct Active {
    target: Handle<Transform>,
    frame: Wireframe,
    resizing: Option<Vec<ResizeKnob>>,
    dragging: Option<Dragging>,
}

struct ResizeKnob {
    wireframe: Wireframe,
    collider: Handle<PointerCollider>,
    dragging: Option<Dragging>,
}

struct Dragging {
    element_base: Position,
    pointer_base: Position,
}

impl Element for TransformTool {
    fn when_inserted(&mut self, world: &World, tool: Handle<Self>) {}
}
