use glam::{IVec2, UVec2};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layout::{
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        packer::Packer,
        transfer::Transfer,
        transform::{Transform, TransformEdge, TransformValue},
        val::{Val, ValChanged},
    },
    measures::IRect,
    theme::{Theme, ThemeVal},
    tools::collider::ToolRectCollider,
    widgets::{
        button::{ButtonAction, ButtonImage, ToggleButton},
        common::WidgetCommon,
        renderer::rrect::RRect,
    },
};

pub struct Tabs {
    pub rect: Handle<Val<IRect>>,
    pub common: WidgetCommon,
    pub tabs: Vec<TabEntry>,
    pub active: Handle<Val<usize>>,
}

pub struct TabEntry {
    pub icon: ButtonImage,
    pub rect: Handle<Val<IRect>>,
    pub visible: Handle<Val<bool>>,
}

impl Tabs {
    pub fn init(&self, world: &World, handle: Handle<Self>) {
        world.scope("tabs");
        world.set_autodeps(Some(handle));
        world.dependency(handle, self.rect);
        world.dependency(handle, self.common.visible);

        let theme = world.single_fetch::<Theme>();
        let theme_val = world.single_fetch::<ThemeVal>();

        let roundness = theme.roundness;
        let shadow_offset = theme.shadow_offset.as_ivec2();
        let shadow_blur = theme.shadow_blur;

        world.insert(RRect {
            common: self.common,
            rect: self.rect,
            order: -10,
            color: theme_val.gray2,
            radius: roundness,
            width: 0.0,
        });

        let shadow_rect = world.build(Transform::new(
            TransformValue::shift(shadow_offset.x, shadow_offset.y),
            self.rect,
        ));
        world.insert(RRect {
            common: self.common,
            rect: shadow_rect,
            order: -11,
            color: theme_val.shadow_color,
            radius: roundness,
            width: shadow_blur,
        });

        world.insert(ToolRectCollider {
            common: self.common,
            target: handle.untyped(),
            rect: self.rect,
            order: -10,
        });

        let side_rect = world.build(Transform::new(side_transform(), self.rect));
        let main_rect = world.build(Transform::new(main_transform(), self.rect));

        let mut luni = LuniFlex::new(
            side_rect,
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
        );

        for (i, entry) in self.tabs.iter().enumerate() {
            // FIXME deep duplication
            let mut icon = entry.icon.clone();
            let color = world.fetch(icon.modulate).val;
            icon.modulate = world.insert(Val::new(color));
            let entry_icon_modulate = icon.modulate;

            let button_rect = world.build(luni.child(LuniChild::default()));
            let button_selected = Transfer::derive(world, self.active).build(move |&x| x == i);
            let button = world.insert(ToggleButton {
                common: self.common,
                rect: button_rect,
                theme: theme_val.tab_button(),
                image: Some(icon),
                selected: button_selected,
                hovering: world.insert(Val::new(false)),
            });

            let indicator_rect = world.build(Transform::new(
                TransformValue::anchor((0.0, 0.5), IRect::new_half(IVec2::ZERO, UVec2::new(1, 16))),
                button_rect,
            ));
            world.insert(RRect {
                common: WidgetCommon {
                    camera: self.common.camera,
                    visible: entry.visible,
                },
                rect: indicator_rect,
                order: 100,
                color: theme_val.theme,
                radius: 0.0,
                width: 0.0,
            });

            Transfer::new(world, main_rect, entry.rect).copy();

            Packer::new(world, entry.visible, move |world| {
                let this = world.fetch(handle);
                world.fetch(button_selected).val && world.fetch(this.common.visible).val
            })
            .listen(button_selected)
            .listen(self.common.visible)
            .finish();

            Packer::new(world, entry_icon_modulate, move |world| {
                let theme = world.single_fetch::<ThemeVal>();
                match world.fetch(button_selected).val {
                    true => world.fetch(theme.distinct).val,
                    false => world.fetch(theme.gray4).val,
                }
            })
            .listen(button_selected)
            .listen(theme_val.distinct)
            .listen(theme_val.gray4)
            .finish();

            world.observer(button, move |action: &ButtonAction, world| {
                if let ButtonAction::Press = action {
                    let this = world.fetch(handle);
                    let mut active = world.fetch_mut(this.active);
                    active.val = i;
                    world.queue_trigger(active.handle(), ValChanged);
                }
            });
        }

        world.insert(luni);
    }
}

fn side_transform() -> TransformValue {
    TransformValue {
        left: TransformEdge {
            anchor: 0.0,
            offset: 0,
        },
        down: TransformEdge {
            anchor: 0.0,
            offset: 0,
        },
        right: TransformEdge {
            anchor: 0.0,
            offset: 48,
        },
        up: TransformEdge {
            anchor: 1.0,
            offset: 0,
        },
    }
}

fn main_transform() -> TransformValue {
    TransformValue {
        left: TransformEdge {
            anchor: 0.0,
            offset: 48,
        },
        down: TransformEdge {
            anchor: 0.0,
            offset: 0,
        },
        right: TransformEdge {
            anchor: 1.0,
            offset: 0,
        },
        up: TransformEdge {
            anchor: 1.0,
            offset: 0,
        },
    }
}

impl Element for Tabs {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
