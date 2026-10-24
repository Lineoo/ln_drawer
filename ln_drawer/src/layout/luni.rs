use ln_world::{Descriptor, Element, Handle, World};

use crate::{
    layout::val::{Val, ValChanged},
    measures::IRect,
};

pub struct LuniFlex {
    pub parent: (Handle<Val<IRect>>, LuniParent),
    pub children: Vec<(Handle<Val<IRect>>, LuniChild)>,
}

#[derive(Default)]
pub struct LuniParent {
    pub axis: LuniAxis,
    pub distribution: LuniDistribution,
    pub padding: LuniRect,
    pub gap: i32,
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
#[expect(unused)]
pub enum LuniAxis {
    #[default]
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

#[derive(Default, Clone, Copy)]
#[expect(unused)]
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
#[expect(unused)]
pub enum LuniAlign {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
}

/// TODO specific certain data for parent to adjust itself based on its children
#[expect(unused)]
pub struct LuniHug;

impl LuniFlex {
    pub fn new(rect: Handle<Val<IRect>>, parent: LuniParent) -> Self {
        LuniFlex {
            parent: (rect, parent),
            children: Vec::new(),
        }
    }

    pub fn child(&mut self, child: LuniChild) -> impl Descriptor<Target = Handle<Val<IRect>>> {
        LuniFlexChildBuilder { luni: self, child }
    }
}

fn compute(rect: IRect, parent: &LuniParent, children: &[&LuniChild]) -> Vec<IRect> {
    let mut result = Vec::with_capacity(children.len());
    let mut lengths = Vec::with_capacity(children.len());

    // First loop - sum up all grow, shrink and required length

    let mut grow_sum = 0.0;
    let mut shrink_sum = 0.0;
    let mut available = parent.gap + rect_main_lenth(rect, parent.axis)
        - rect_main_padding(parent.padding, parent.axis);
    for child in children {
        let child = child.apply(&parent.template);
        grow_sum += child.grow;
        shrink_sum += child.shrink * child.basis as f32;
        available -= child.basis + parent.gap + rect_main_margin(child.margin, parent.axis);
        lengths.push(child.basis);
    }

    // Second loop - calculate and distribute real length

    let mut distributed = 0;
    let mut i = 0;
    while i < children.len() {
        let child = &children[i];
        let child = child.apply(&parent.template);

        let delta = if available > 0 && child.grow > 0.0 {
            (available as f32 * (child.grow / grow_sum)).round() as i32
        } else if available < 0 && child.shrink > 0.0 {
            (available as f32 * (child.shrink * child.basis as f32 / shrink_sum)).round() as i32
        } else {
            0
        };

        lengths[i] += delta;
        distributed += delta;
        i += 1;
    }

    // Third loop - assign final position

    let (offset, extra) = distribute(available - distributed, children.len(), parent.distribution);

    let mut cursor = rect_main_start(parent.padding, parent.axis) + offset;
    for (i, child) in children.iter().enumerate() {
        let child = child.apply(&parent.template);
        let length = lengths[i];
        result.push(cursor_assign(
            cursor,
            rect,
            parent.padding,
            child.margin,
            child.cross,
            length,
            child.align,
            parent.axis,
        ));

        cursor += length + parent.gap + extra + rect_main_margin(child.margin, parent.axis);
    }

    result
}

/// Remaining space at the start, plus extra gap between adjacent children.
fn distribute(free: i32, count: usize, distr: LuniDistribution) -> (i32, i32) {
    let free = free.max(0);
    let count = count as i32;
    match distr {
        LuniDistribution::FlexStart => (0, 0),
        LuniDistribution::FlexEnd => (free, 0),
        LuniDistribution::Center => (free / 2, 0),
        LuniDistribution::SpaceBetween if count > 1 => (0, free / (count - 1)),
        LuniDistribution::SpaceBetween => (0, 0),
        LuniDistribution::SpaceAround if count > 0 => {
            let unit = free / count;
            (unit / 2, unit)
        }
        LuniDistribution::SpaceAround => (0, 0),
        LuniDistribution::SpaceEvenly => {
            let unit = free / (count + 1);
            (unit, unit)
        }
    }
}

fn rect_main_lenth(rect: IRect, axis: LuniAxis) -> i32 {
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
    rect: IRect,
    padding: LuniRect,
    margin: LuniRect,
    cross: i32,
    length: i32,
    align: LuniAlign,
    axis: LuniAxis,
) -> IRect {
    match align {
        LuniAlign::Stretch => match axis {
            LuniAxis::Row => IRect::new(
                rect.left() + cursor + margin.left,
                rect.down() + padding.bottom + margin.bottom,
                rect.left() + cursor + margin.left + length,
                rect.up() - padding.top - margin.top,
            ),
            LuniAxis::RowReverse => IRect::new(
                rect.right() - cursor - margin.right,
                rect.down() + padding.bottom + margin.bottom,
                rect.right() - cursor - margin.right - length,
                rect.up() - padding.top - margin.top,
            ),
            LuniAxis::Column => IRect::new(
                rect.left() + padding.left + margin.left,
                rect.up() - cursor - margin.top - length,
                rect.right() - padding.right - margin.right,
                rect.up() - cursor - margin.top,
            ),
            LuniAxis::ColumnReverse => IRect::new(
                rect.left() + padding.left + margin.left,
                rect.down() + cursor + margin.top,
                rect.right() - padding.right - margin.right,
                rect.down() + cursor + margin.top + length,
            ),
        },
        LuniAlign::FlexStart => match axis {
            LuniAxis::Row => IRect::new(
                rect.left() + cursor + margin.left,
                rect.up() - padding.top - margin.top - cross,
                rect.left() + cursor + margin.left + length,
                rect.up() - padding.top - margin.top,
            ),
            LuniAxis::RowReverse => IRect::new(
                rect.right() - cursor - margin.right,
                rect.up() - padding.top - margin.top - cross,
                rect.right() - cursor - margin.right - length,
                rect.up() - padding.top - margin.top,
            ),
            LuniAxis::Column => IRect::new(
                rect.left() + padding.left + margin.left,
                rect.up() - cursor - margin.top - length,
                rect.left() + padding.left + margin.left + cross,
                rect.up() - cursor - margin.top,
            ),
            LuniAxis::ColumnReverse => IRect::new(
                rect.left() + padding.left + margin.left,
                rect.down() + cursor + margin.top,
                rect.left() + padding.left + margin.left + cross,
                rect.down() + cursor + margin.top + length,
            ),
        },
        LuniAlign::FlexEnd => match axis {
            LuniAxis::Row => IRect::new(
                rect.left() + cursor + margin.left,
                rect.down() + padding.bottom + margin.bottom,
                rect.left() + cursor + margin.left + length,
                rect.down() + padding.bottom + margin.bottom + cross,
            ),
            LuniAxis::RowReverse => IRect::new(
                rect.right() - cursor - margin.right,
                rect.down() + padding.bottom + margin.bottom,
                rect.right() - cursor - margin.right - length,
                rect.down() + padding.bottom + margin.bottom + cross,
            ),
            LuniAxis::Column => IRect::new(
                rect.right() - padding.right - margin.right - cross,
                rect.up() - cursor - margin.top - length,
                rect.right() - padding.right - margin.right,
                rect.up() - cursor - margin.top,
            ),
            LuniAxis::ColumnReverse => IRect::new(
                rect.right() - padding.right - margin.right - cross,
                rect.down() + cursor + margin.top,
                rect.right() - padding.right - margin.right,
                rect.down() + cursor + margin.top + length,
            ),
        },
        LuniAlign::Center => match axis {
            LuniAxis::Row => IRect::new(
                rect.left() + cursor + margin.left,
                rect.vertical_center() - cross / 2,
                rect.left() + cursor + margin.left + length,
                rect.vertical_center() + cross / 2,
            ),
            LuniAxis::RowReverse => IRect::new(
                rect.right() - cursor - margin.right,
                rect.vertical_center() - cross / 2,
                rect.right() - cursor - margin.right - length,
                rect.vertical_center() + cross / 2,
            ),
            LuniAxis::Column => IRect::new(
                rect.horizontal_center() - cross / 2,
                rect.up() - cursor - margin.top - length,
                rect.horizontal_center() + cross / 2,
                rect.up() - cursor - margin.top,
            ),
            LuniAxis::ColumnReverse => IRect::new(
                rect.horizontal_center() - cross / 2,
                rect.down() + cursor + margin.top,
                rect.horizontal_center() + cross / 2,
                rect.down() + cursor + margin.top + length,
            ),
        },
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

struct LuniFlexChildBuilder<'a> {
    luni: &'a mut LuniFlex,
    child: LuniChild,
}

impl Descriptor for LuniFlexChildBuilder<'_> {
    type Target = Handle<Val<IRect>>;

    fn when_build(self, world: &World) -> Self::Target {
        let rect = world.insert(Val::new(IRect::default()));
        self.luni.children.push((rect, self.child));
        rect
    }
}

impl Element for LuniFlex {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("luni_flex");
        world.set_autodeps(Some(this));

        world.observer(self.parent.0, move |ValChanged, world| {
            let this = world.fetch(this);
            let parent_rect = world.fetch(this.parent.0).val;
            let children = this.children.iter().map(|x| &x.1).collect::<Vec<_>>();
            let results = compute(parent_rect, &this.parent.1, &children[..]);

            for (&result, child_rect) in (results.iter()).zip(this.children.iter().map(|x| x.0)) {
                let mut child_rect = world.fetch_mut(child_rect);
                child_rect.val = result;
                world.queue_trigger(child_rect.handle(), ValChanged);
            }
        });
    }
}
