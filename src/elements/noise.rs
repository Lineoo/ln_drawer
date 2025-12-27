use crate::{measures::Position, world::{Descriptor, Handle, World}};

pub struct SimpleNoise {
    pub position: Position,
}

pub struct SimpleNoiseDescriptor {
    pub position: Position,
}

impl Descriptor for SimpleNoiseDescriptor {
    type Target = Handle<SimpleNoise>;

    fn when_build(self, world: &World) -> Self::Target {
        todo!()
    }
}