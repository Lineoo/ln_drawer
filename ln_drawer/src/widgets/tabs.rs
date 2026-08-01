use ln_world::{Descriptor, Element, Handle, World};

use crate::{
    layout::luni::{
        LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex, LuniParent,
        LuniRect,
    },
    measures::Rectangle,
    render::rounded::RoundedRectDescriptor,
    theme::Theme,
    widgets::{
        ButtonAction, SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
        button::{ButtonImage, SetButtonSelected, ToggleButton, ToggleButtonTheme},
    },
};

pub struct TabsActive(pub usize);
pub struct SetTabsActive(pub usize);

pub struct Tabs {
    pub rect: Rectangle,
    pub visible: bool,
    pub tabs: Vec<(ButtonImage, Handle)>,
    pub active: usize,
}

impl Tabs {
    pub fn build(self, world: &World) -> Handle<Tabs> {
        let theme = world.single_fetch::<Theme>().unwrap();

        let back = world.build(RoundedRectDescriptor {
            rect: self.rect,
            color: theme.secondary_color,
            shadow_color: theme.shadow_color,
            shadow_offset: theme.shadow_offset,
            shadow_blur: theme.shadow_blur,
            shrink: theme.roundness,
            value: theme.roundness,
            vertex_extend: 20,
            visible: true,
            order: -10,
        });

        let mut children = Vec::new();
        let mut luni_children = Vec::new();
        for (entry, _) in &self.tabs {
            let button = world.build(ToggleButton {
                rect: self.rect,
                theme: ToggleButtonTheme {
                    idle_color: theme.secondary_color,
                    hover_color: theme.highlight_color,
                    press_color: theme.blank_color,
                    selected_color: theme.blank_color,
                },
                image: Some(entry.clone()),
                selected: false,
                visible: true,
                hovering: false,
            });

            children.push(button);
            luni_children.push((button.untyped(), LuniChild::default()));
        }

        let side = world.insert(());

        world.insert(LuniFlex {
            parent: (
                side.untyped(),
                LuniParent {
                    axis: LuniAxis::Column,
                    distribution: LuniDistribution::FlexStart,
                    padding: LuniRect {
                        left: 0,
                        bottom: 4,
                        right: 0,
                        top: 4,
                    },
                    gap: 0,
                    template: LuniChildTemplate {
                        align: LuniAlign::FlexStart,
                        margin: LuniRect {
                            left: 4,
                            bottom: 4,
                            right: 4,
                            top: 4,
                        },
                        basis: 40,
                        max: None,
                        min: None,
                        grow: 0.0,
                        shrink: 0.0,
                        cross: 40,
                    },
                },
            ),
            children: luni_children,
        });

        let handle = world.insert(self);

        for (i, &child) in children.iter().enumerate() {
            world.observer(child, move |action: &ButtonAction, world| {
                if let ButtonAction::Press = action {
                    world.queue_trigger(handle, SetTabsActive(i));
                }
            });
        }

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back = world.fetch_mut(back).unwrap();

            this.rect = rect;
            back.desc.rect = rect;

            world.queue_trigger(handle, WidgetRectangle(main_rect(rect)));
            world.queue_trigger(side, WidgetRectangle(side_rect(rect)));

            for &(_, entry) in &this.tabs {
                world.queue_trigger(entry, SetWidgetRectangle(main_rect(rect)));
            }
        });

        world.observer(handle, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut back = world.fetch_mut(back).unwrap();

            this.visible = visible;
            back.desc.visible = visible;

            world.queue_trigger(handle, WidgetVisible(visible));
            world.queue_trigger(side, WidgetVisible(visible));

            for (i, &(_, entry)) in this.tabs.iter().enumerate() {
                world.queue_trigger(entry, SetWidgetVisible(visible && i == this.active));
            }
        });

        world.observer(handle, move |&SetTabsActive(active), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            this.active = active;

            for (i, &child) in children.iter().enumerate() {
                world.queue_trigger(child, SetButtonSelected(i == active));
            }

            for (i, &(_, entry)) in this.tabs.iter().enumerate() {
                world.queue_trigger(entry, SetWidgetVisible(this.visible && i == active));
            }
        });

        // initialize layout
        world.queue(move |world| {
            let this = world.fetch(handle).unwrap();
            world.queue_trigger(handle, SetWidgetRectangle(this.rect));
            world.queue_trigger(handle, SetWidgetVisible(this.visible));
            world.queue_trigger(handle, SetTabsActive(this.active));
        });

        handle
    }
}

fn side_rect(rect: Rectangle) -> Rectangle {
    rect.with_right(rect.left() + 48)
}

fn main_rect(rect: Rectangle) -> Rectangle {
    rect.with_left(rect.left() + 48)
}

impl Element for Tabs {}
impl Descriptor for Tabs {
    type Target = Handle<Tabs>;
    fn when_build(self, world: &World) -> Self::Target {
        self.build(world)
    }
}
