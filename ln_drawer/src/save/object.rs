use glam::UVec2;
use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use uuid::Uuid;

use crate::{
    layout::val::Val,
    measures::Rectangle,
    save::{Autosave, Save},
    widgets::{
        common::WidgetCommon,
        object::color_snip::{ColorSnip, ColorSnipAttr},
    },
};

pub const TABLE_LAYER_OBJECT: TableDefinition<(u64, u128), &[u8]> =
    TableDefinition::new("layer_object");

pub struct Objects {
    pub common: WidgetCommon,
}

impl Element for Objects {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("objects");
        world.set_autodeps(Some(this));
        world.dependency(this, self.common.visible);

        let save = world.single_fetch::<Save>();
        let read = save.db.begin_read().unwrap();
        let table = read.open_table(TABLE_LAYER_OBJECT).unwrap();
        for entry in table.iter().unwrap() {
            let (k, v) = entry.unwrap();
            let ((_page, uuid), bytes) = (k.value(), v.value());
            let handle = match serde_json::from_slice::<ObjectData>(bytes).unwrap() {
                ObjectData::ColorSnip(attr) => {
                    let rect = world.insert(Val::new(Rectangle::new_half(
                        attr.position,
                        UVec2::splat(24),
                    )));
                    ObjectHandle::ColorSnip(world.insert(ColorSnip {
                        common: self.common,
                        rect,
                        attr,
                    }))
                }
            };
            world.insert(Object {
                uuid: Uuid::from_u128(uuid),
                handle,
            });
        }

        world.insert(Autosave(Box::new(|world, task| {
            let mut table = task.write.open_table(TABLE_LAYER_OBJECT).unwrap();
            world.foreach_fetch::<Object>(|obj| {
                let data = match obj.handle {
                    ObjectHandle::ColorSnip(handle) => {
                        let snip = world.fetch(handle);
                        ObjectData::ColorSnip(snip.attr)
                    }
                };
                let bytes = serde_json::to_vec(&data).unwrap();
                table.insert((0, obj.uuid.as_u128()), &bytes[..]).unwrap();
            });
        })));
    }
}

pub struct Object {
    pub uuid: Uuid,
    pub handle: ObjectHandle,
}

impl Element for Object {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectHandle {
    ColorSnip(Handle<ColorSnip>),
}

impl HandleGeneric for ObjectHandle {
    fn untyped(self) -> HandleAny {
        match self {
            ObjectHandle::ColorSnip(handle) => handle.untyped(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum ObjectData {
    ColorSnip(ColorSnipAttr),
}
