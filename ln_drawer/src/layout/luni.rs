use ln_world::{Element, Handle, World};

use crate::{
    measures::Rectangle,
    widgets::{WidgetAnimatedRectangle, WidgetRectangle},
};

pub struct LuniFlex {
    pub parent: (Handle, LuniParent),
    pub children: Vec<(Handle, LuniChild)>,
}

#[derive(Default)]
pub struct LuniParent {
    pub axis: LuniAxis,
    pub distribution: LuniDistribution,
    pub padding: LuniRect,
    pub template: LuniChildTemplate,
}

#[derive(Default)]
pub struct LuniChildTemplate {
    pub align: LuniAlign,
    pub margin: LuniRect,
    pub basis: i32,
    pub max: Option<i32>,
    pub min: Option<i32>,
    pub grow: f32,
    pub shrink: f32,
    pub cross: i32,
}

#[derive(Default)]
pub struct LuniChild {
    pub align: Option<LuniAlign>,
    pub margin: Option<LuniRect>,
    pub basis: Option<i32>,
    pub max: Option<Option<i32>>,
    pub min: Option<Option<i32>>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    pub cross: Option<i32>,
}

#[derive(Default, Clone, Copy)]
pub enum LuniAxis {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Default, Clone, Copy)]
pub enum LuniDistribution {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Default, Clone, Copy)]
pub struct LuniRect {
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub top: i32,
}

#[derive(Default, Clone, Copy)]
pub enum LuniAlign {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
}

impl LuniFlex {
    fn compute(&self, rect: Rectangle) -> Vec<(Handle, Rectangle)> {
        let mut result = Vec::with_capacity(self.children.len());
        let mut lengths = Vec::with_capacity(self.children.len());

        let (_, parent) = &self.parent;

        let mut grow_sum = 0.0;
        let mut shrink_sum = 0.0;
        let mut available = rect.width() as i32 - parent.padding.left - parent.padding.right;
        for (_, child) in &self.children {
            let child = child.apply(&parent.template);
            grow_sum += child.grow;
            shrink_sum += child.shrink * child.basis as f32;
            available -= child.basis + child.margin.left + child.margin.right;
            lengths.push(child.basis);
        }

        let mut i = 0;
        while i < self.children.len() {
            let (_, child) = &self.children[i];
            let child = child.apply(&parent.template);

            if available > 0 && child.grow > 0.0 {
                lengths[i] += (available as f32 * (child.grow / grow_sum)).round() as i32;
            } else if available < 0 && child.shrink > 0.0 {
                lengths[i] += (available as f32 * (child.shrink * child.basis as f32 / shrink_sum))
                    .round() as i32;
            }

            i += 1;
        }

        let mut cursor = parent.padding.left;
        for (i, (handle, child)) in self.children.iter().enumerate() {
            let child = child.apply(&parent.template);
            let (bottom, top) = match child.align {
                LuniAlign::Stretch => (parent.padding.bottom, parent.padding.top),
                LuniAlign::FlexStart => todo!(),
                LuniAlign::FlexEnd => todo!(),
                LuniAlign::Center => todo!(),
            };

            let length = lengths[i];

            result.push((
                *handle,
                Rectangle::new(
                    rect.left() + cursor + child.margin.left,
                    rect.down() + bottom + child.margin.bottom,
                    rect.left() + cursor + child.margin.left + length,
                    rect.up() - top - child.margin.top,
                ),
            ));

            cursor += child.margin.right + length;
        }

        result
    }
}

impl LuniChild {
    fn apply(&self, template: &LuniChildTemplate) -> LuniChildTemplate {
        LuniChildTemplate {
            align: self.align.unwrap_or(template.align),
            margin: self.margin.unwrap_or(template.margin),
            basis: self.basis.unwrap_or(template.basis),
            grow: self.grow.unwrap_or(template.grow),
            shrink: self.shrink.unwrap_or(template.shrink),
            max: self.max.unwrap_or(template.max),
            min: self.min.unwrap_or(template.min),
            cross: self.cross.unwrap_or(template.cross),
        }
    }
}

impl Element for LuniFlex {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let ob = world.observer(self.parent.0, move |&WidgetRectangle(rect), world| {
            let this = world.fetch(this).unwrap();
            let targets = this.compute(rect);

            for (child, target) in targets {
                world.queue_trigger(child, WidgetRectangle(target));
            }
        });

        let oba = world.observer(
            self.parent.0,
            move |&WidgetAnimatedRectangle(rect), world| {
                let this = world.fetch(this).unwrap();
                let targets = this.compute(rect);

                for (child, target) in targets {
                    world.queue_trigger(child, WidgetAnimatedRectangle(target));
                }
            },
        );

        world.dependency(ob, this);
        world.dependency(oba, this);
        world.dependency(this, self.parent.0);
        for (child, _) in &self.children {
            world.dependency(this, *child);
        }
    }
}
