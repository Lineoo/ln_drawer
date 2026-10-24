use cosmic_text::{Attrs, Metrics};
use ln_world::{Element, Handle, World};

use crate::{
    i18n::tr,
    layer::wrapper::{BrushConfigurationChanged, LayerPage},
    layout::{
        anim::Animator,
        luni::{
            LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniDistribution, LuniFlex,
            LuniParent, LuniRect,
        },
        packer::Packer,
        transform::{Transform, TransformEdge, TransformValue},
        val::Val,
    },
    measures::IRect,
    theme::{Theme, ThemeVal},
    tools::pointer::{PointerHit, PointerHover, PointerScroll},
    widgets::{
        brush_preview::BrushPreview,
        button::{
            ButtonClick, ButtonDrag, ButtonDragStatus, ButtonImage, SetButtonSelected,
            ToggleButton, ToggleButtonTheme,
        },
        common::WidgetCommon,
        container::{ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerWithCamera},
        panel::settings::SettingPanel,
        renderer::{
            rrect::RRect,
            svg::svg_render,
            text::{Text, TextAttr},
        },
        tabs::{TabEntry, Tabs},
    },
};

const ITEM_HEIGHT: i32 = 80;
const ITEM_GAP: i32 = 6;
const LIST_PADDING: i32 = 8;

pub struct BrushPanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
}

impl Element for BrushPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("brush_panel");
        world.set_autodeps(Some(this));

        let theme = world.single_fetch::<ThemeVal>();

        let list_rect = world.insert(Val::new(IRect::default()));
        let list_visible = world.insert(Val::new(false));
        world.insert(BrushListPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: list_visible,
            },
            rect: list_rect,
        });

        let settings_rect = world.insert(Val::new(IRect::default()));
        let settings_visible = world.insert(Val::new(false));
        world.insert(SettingPanel {
            common: WidgetCommon {
                camera: self.common.camera,
                visible: settings_visible,
            },
            rect: settings_rect,
        });

        world.insert(Tabs {
            common: self.common,
            rect: self.rect,
            tabs: vec![
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/brush.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: list_rect,
                    visible: list_visible,
                },
                TabEntry {
                    icon: ButtonImage::new(
                        svg_render(include_bytes!("../../../res/interface/settings.svg")),
                        theme.distinct,
                        12,
                    ),
                    rect: settings_rect,
                    visible: settings_visible,
                },
            ],
            active: world.insert(Val::new(0)),
        });
    }
}

pub struct BrushListPanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
}

impl Element for BrushListPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("brush_list_panel");
        world.set_autodeps(Some(this));

        let theme = world.single_fetch::<Theme>();
        let theme_val = world.single_fetch::<ThemeVal>();
        let page = world.single_fetch::<LayerPage>();

        let count = page.brushes.len();
        let active = page.active_brush;

        let content_height =
            count as i32 * ITEM_HEIGHT + (count as i32 - 1).max(0) * ITEM_GAP + LIST_PADDING * 2;

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let restriction = world.build(ContainerDecoDropdown {
            height: Some(content_height),
            outer: self.rect,
        });
        world.insert(ContainerDeco {
            container,
            restriction,
        });

        let mut luni = LuniFlex::new(
            restriction,
            LuniParent {
                axis: LuniAxis::Column,
                distribution: LuniDistribution::FlexStart,
                padding: LuniRect {
                    left: 8,
                    bottom: LIST_PADDING,
                    right: 8,
                    top: LIST_PADDING,
                },
                gap: ITEM_GAP,
                template: LuniChildTemplate {
                    align: LuniAlign::Stretch,
                    basis: ITEM_HEIGHT,
                    ..Default::default()
                },
            },
        );

        for i in 0..count {
            let button_rect = world.build(luni.child(LuniChild::default()));
            let selected = world.insert(Val::new(i == active));
            let button = world.insert(ToggleButton {
                common,
                rect: button_rect,
                theme: ToggleButtonTheme {
                    idle_color: theme_val.gray2,
                    hover_color: theme_val.gray3,
                    press_color: theme_val.gray3,
                    selected_color: theme_val.gray2,
                },
                image: None,
                selected,
                hovering: world.insert(Val::new(false)),
            });

            let outline_rect =
                world.build(Transform::new(TransformValue::shrink(-2, -2), button_rect));
            let outline_color_anim = Packer::derive(world, move |world| {
                let theme = world.single_fetch::<ThemeVal>();
                let selected_val = world.fetch(selected).val;
                match selected_val {
                    true => world.fetch(theme.theme).val,
                    false => world.fetch(theme.gray2).val,
                }
            })
            .listen(selected)
            .listen(theme_val.theme)
            .listen(theme_val.gray2)
            .finish();
            let outline_color = world.build(Animator::new(outline_color_anim, theme.anim_factor));
            world.insert(RRect {
                common,
                rect: outline_rect,
                order: 9,
                color: outline_color,
                radius: theme.roundness,
                width: 0.0,
            });

            let label_rect = world.build(Transform::new(
                TransformValue {
                    left: TransformEdge {
                        anchor: 0.0,
                        offset: 14,
                    },
                    down: TransformEdge {
                        anchor: 1.0,
                        offset: -4 - 14,
                    },
                    right: TransformEdge {
                        anchor: 1.0,
                        offset: -14,
                    },
                    up: TransformEdge {
                        anchor: 1.0,
                        offset: -4,
                    },
                },
                button_rect,
            ));
            world.insert(Text {
                attr: TextAttr {
                    metrics: Metrics {
                        font_size: 12.0,
                        line_height: 14.0,
                    },
                    attrs: Attrs::new(),
                    ..Default::default()
                },
                text: tr(page.brushes[i].label).into(),
                rect: label_rect,
                order: 150,
                common,
                modulate: theme_val.distinct,
                outdated: false,
                canvas_outdated: false,
            });

            let preview_rect = world.build(Transform::new(
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
                        anchor: 1.0,
                        offset: 0,
                    },
                    up: TransformEdge {
                        anchor: 1.0,
                        offset: -20,
                    },
                },
                button_rect,
            ));
            let preview = world.insert(BrushPreview {
                common,
                rect: preview_rect,
                outdated: true,
                brush: page.brushes.get(i).map(|x| x.brush.dup()),
            });

            world.observer(button, move |&ButtonClick, world| {
                let mut page = world.single_fetch_mut::<LayerPage>();
                if page.active_brush == i {
                    return;
                }
                page.select_brush(i);
                world.queue_trigger(page.handle(), BrushConfigurationChanged);
            });

            world.observer(page.handle(), move |&BrushConfigurationChanged, world| {
                let page = world.single_fetch::<LayerPage>();
                let active = page.active_brush;
                let selected = i == active;
                world.queue_trigger(button, SetButtonSelected(selected));
            });

            world.observer(button, move |drag: &ButtonDrag, world| match drag.status {
                ButtonDragStatus::Start => {
                    world.trigger(
                        container,
                        &PointerHit {
                            position: drag.here.position,
                            pointer: drag.here.pointer,
                            status: drag.from.status,
                            data: drag.here.data,
                        },
                    );
                }
                ButtonDragStatus::Dragging | ButtonDragStatus::End => {
                    world.trigger(
                        container,
                        &PointerHit {
                            position: drag.here.position,
                            pointer: drag.here.pointer,
                            status: drag.here.status,
                            data: drag.here.data,
                        },
                    );
                }
            });

            // TODO temporary: `Container` couples render order with collider order, so the preview
            // captures pointer events. Forward them to the button until that is decoupled.
            world.observer(preview, move |event: &PointerHit, world| {
                world.queue_trigger(button, *event);
            });
            world.observer(preview, move |event: &PointerHover, world| {
                world.queue_trigger(button, *event);
            });
            world.observer(preview, move |event: &PointerScroll, world| {
                world.queue_trigger(button, *event);
            });
            world.observer(button, move |event: &PointerScroll, world| {
                world.queue_trigger(container, *event);
            });
        }

        world.insert(luni);
    }
}
