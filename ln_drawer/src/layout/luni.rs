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

/// TODO specific certain data for parent to adjust itself based on its children
pub struct LuniHug;

impl LuniFlex {
    fn compute(&self, rect: Rectangle) -> Vec<(Handle, Rectangle)> {
        let mut result = Vec::with_capacity(self.children.len());
        let mut lengths = Vec::with_capacity(self.children.len());

        let (_, parent) = &self.parent;

        let mut grow_sum = 0.0;
        let mut shrink_sum = 0.0;
        let mut available =
            rect_main_lenth(rect, parent.axis) - rect_main_padding(parent.padding, parent.axis);
        for (_, child) in &self.children {
            let child = child.apply(&parent.template);
            grow_sum += child.grow;
            shrink_sum += child.shrink * child.basis as f32;
            available -= child.basis + rect_main_margin(child.margin, parent.axis);
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

        let mut cursor = rect_main_start(parent.padding, parent.axis);
        for (i, (handle, child)) in self.children.iter().enumerate() {
            let child = child.apply(&parent.template);
            let length = lengths[i];
            result.push((
                *handle,
                cursor_assign(
                    cursor,
                    rect,
                    parent.padding,
                    child.margin,
                    length,
                    child.align,
                    parent.axis,
                ),
            ));

            cursor += cursor_step(child.margin, parent.axis) + length;
        }

        result
    }
}

fn rect_main_lenth(rect: Rectangle, axis: LuniAxis) -> i32 {
    match axis {
        LuniAxis::Column | LuniAxis::ColumnReverse => rect.height() as i32,
        LuniAxis::Row | LuniAxis::RowReverse => rect.width() as i32,
    }
}

fn rect_main_padding(padding: LuniRect, axis: LuniAxis) -> i32 {
    match axis {
        LuniAxis::Column | LuniAxis::ColumnReverse => padding.top + padding.bottom,
        LuniAxis::Row | LuniAxis::RowReverse => padding.left + padding.right,
    }
}

fn rect_main_margin(margin: LuniRect, axis: LuniAxis) -> i32 {
    match axis {
        LuniAxis::Column | LuniAxis::ColumnReverse => margin.top + margin.bottom,
        LuniAxis::Row | LuniAxis::RowReverse => margin.left + margin.right,
    }
}

fn rect_main_start(padding: LuniRect, axis: LuniAxis) -> i32 {
    match axis {
        LuniAxis::Row => padding.left,
        LuniAxis::RowReverse => padding.right,
        LuniAxis::Column => padding.top,
        LuniAxis::ColumnReverse => padding.bottom,
    }
}

fn cursor_assign(
    cursor: i32,
    rect: Rectangle,
    padding: LuniRect,
    margin: LuniRect,
    length: i32,
    align: LuniAlign,
    axis: LuniAxis,
) -> Rectangle {
    let (start, end) = match align {
        LuniAlign::Stretch => (padding.bottom, padding.top),
        LuniAlign::FlexStart => todo!(),
        LuniAlign::FlexEnd => todo!(),
        LuniAlign::Center => todo!(),
    };

    match axis {
        LuniAxis::Row => Rectangle::new(
            rect.left() + cursor + margin.left,
            rect.down() + start + margin.bottom,
            rect.left() + cursor + margin.left + length,
            rect.up() - end - margin.top,
        ),
        LuniAxis::RowReverse => Rectangle::new(
            rect.right() - cursor - margin.right,
            rect.down() + start + margin.bottom,
            rect.right() - cursor - margin.right - length,
            rect.up() - end - margin.top,
        ),
        LuniAxis::Column => Rectangle::new(
            rect.left() + start + margin.left,
            rect.up() - cursor - margin.top - length,
            rect.right() - end - margin.right,
            rect.up() - cursor - margin.top,
        ),
        LuniAxis::ColumnReverse => Rectangle::new(
            rect.left() + start + margin.left,
            rect.down() + cursor + margin.top,
            rect.right() - end - margin.right,
            rect.down() + cursor + margin.top + length,
        ),
    }
}

fn cursor_step(margin: LuniRect, axis: LuniAxis) -> i32 {
    match axis {
        LuniAxis::Row => margin.right,
        LuniAxis::RowReverse => margin.left,
        LuniAxis::Column => margin.bottom,
        LuniAxis::ColumnReverse => margin.top,
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
