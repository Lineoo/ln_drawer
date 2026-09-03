use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, HandleAny, HandleGeneric, World};

use crate::{
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        transform::{Transform, TransformValue},
    },
    measures::Rectangle,
    theme::Theme,
    tools::collider::ToolCollider,
    widgets::{
        SetWidgetRectangle, SetWidgetVisible, WidgetRectangle, WidgetVisible,
        button::{
            ButtonAction, ButtonImage, SetButtonIconColor, SetButtonSelected, ToggleButton,
            ToggleButtonTheme,
        },
        echo::Echo,
        renderer::rrect::RRect,
    },
};

#[expect(unused)]
pub struct TabsActive(pub usize);
pub struct SetTabsActive(pub usize);

pub struct Tabs {
    pub rect: Rectangle,
    pub visible: bool,
    pub tabs: Vec<(ButtonImage, HandleAny)>,
    pub active: usize,
}

#[derive(Clone)]
struct TabButton {
    button: Handle<ToggleButton>,
    edge: Handle<RRect>,
}

impl Tabs {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        let theme = world.single_fetch::<Theme>().unwrap();

        let roundness = theme.roundness;
        let shadow_offset = theme.shadow_offset.as_ivec2();
        let shadow_blur = theme.shadow_blur;

        let back = world.insert(RRect {
            rect: self.rect,
            order: -10,
            color: theme.secondary_color,
            radius: roundness,
            width: 0.0,
            enabled: true,
        });

        let back_shadow = world.insert(RRect {
            rect: self.rect + shadow_offset,
            order: -11,
            color: theme.shadow_color,
            radius: roundness,
            width: shadow_blur,
            enabled: true,
        });

        let mut children = Vec::new();
        let mut luni_children = Vec::new();
        for (entry, _) in &self.tabs {
            let button = world.insert(ToggleButton {
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

            Echo::new(world, button).widget_rectangle();

            let edge = world.insert(RRect {
                rect: Rectangle::default(),
                order: 100,
                color: theme.theme_color,
                radius: 0.0,
                width: 0.0,
                enabled: false,
            });

            world.insert(Transform {
                value: TransformValue::anchor(
                    (0.0, 0.5),
                    Rectangle::new_half(IVec2::ZERO, UVec2::new(1, 16)),
                ),
                source: button.untyped(),
                target: edge.untyped(),
            });

            children.push(TabButton { button, edge });

            luni_children.push((button.untyped(), LuniChild::default()));
        }

        let collider = world.insert(ToolCollider {
            rect: self.rect,
            order: -10,
            enabled: self.visible,
        });

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

        for (i, tab) in children.iter().enumerate() {
            world.observer(tab.button, move |action: &ButtonAction, world| {
                if let ButtonAction::Press = action {
                    world.queue_trigger(handle, SetTabsActive(i));
                }
            });
        }

        world.observer(handle, move |&SetWidgetRectangle(rect), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();

            this.rect = rect;
            world.queue_trigger(back, SetWidgetRectangle(rect));
            world.queue_trigger(back_shadow, SetWidgetRectangle(rect + shadow_offset));
            collider.rect = rect;

            world.queue_trigger(handle, WidgetRectangle(main_rect(rect)));
            world.queue_trigger(side, WidgetRectangle(side_rect(rect)));

            for &(_, entry) in &this.tabs {
                world.queue_trigger(entry, SetWidgetRectangle(main_rect(rect)));
            }
        });

        let children2 = children.clone();
        world.observer(handle, move |&SetWidgetVisible(visible), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let mut collider = world.fetch_mut(collider).unwrap();

            this.visible = visible;
            world.queue_trigger(back, SetWidgetVisible(visible));
            world.queue_trigger(back_shadow, SetWidgetVisible(visible));
            collider.enabled = visible;

            for (i, tab) in children2.iter().enumerate() {
                world.queue_trigger(tab.edge, SetWidgetVisible(visible && i == this.active));
            }

            world.queue_trigger(handle, WidgetVisible(visible));
            world.queue_trigger(side, WidgetVisible(visible));

            for (i, &(_, entry)) in this.tabs.iter().enumerate() {
                world.queue_trigger(entry, SetWidgetVisible(visible && i == this.active));
            }
        });

        world.observer(handle, move |&SetTabsActive(active), world| {
            let mut this = world.fetch_mut(handle).unwrap();
            let theme = world.single_fetch::<Theme>().unwrap();
            this.active = active;

            for (i, tab) in children.iter().enumerate() {
                world.queue_trigger(tab.button, SetButtonSelected(i == active));
                world.queue_trigger(tab.edge, SetWidgetVisible(this.visible && i == active));
                world.queue_trigger(
                    tab.button,
                    SetButtonIconColor(match i == active {
                        true => theme.symbolic_color,
                        false => theme.significant_color,
                    }),
                );
            }

            for (i, &(_, entry)) in this.tabs.iter().enumerate() {
                world.queue_trigger(entry, SetWidgetVisible(this.visible && i == active));
            }
        });
    }
}

fn side_rect(rect: Rectangle) -> Rectangle {
    rect.with_right(rect.left() + 48)
}

fn main_rect(rect: Rectangle) -> Rectangle {
    rect.with_left(rect.left() + 48)
}

impl Element for Tabs {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
