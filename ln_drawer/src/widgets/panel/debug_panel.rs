use cosmic_text::{Attrs, Family, Metrics};
use glam::I64Vec2;
use ln_world::{Element, Handle, World};

use crate::{
    layer::wrapper::{LayerDebugMessage, LayerPage},
    layout::{
        transform::{Transform, TransformValue},
        val::Val,
    },
    measures::IRect,
    render::{
        Render,
        camera::{PageCamera, PageCameraUtils},
    },
    save::{Save, SceneDocument},
    theme::ThemeVal,
    widgets::{
        button::{ButtonClick, ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton},
        common::WidgetCommon,
        container::{
            ContainerAttr, ContainerDeco, ContainerDecoDropdown, ContainerEcho, ContainerWithCamera,
        },
        renderer::{
            svg::svg_render,
            text::{Text, TextAttr},
        },
    },
};

pub struct DebugPanel {
    pub common: WidgetCommon,
    pub rect: Handle<Val<IRect>>,
}

impl Element for DebugPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.scope("debug_panel");
        world.set_autodeps(Some(this));
        world.dependency(this, self.rect);
        world.dependency(this, self.common.visible);

        let theme = world.single_fetch::<ThemeVal>();

        let attr = ContainerAttr::default();
        let (container, common) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            attr,
        });
        let restriction = world.build(ContainerDecoDropdown {
            height: None,
            outer: self.rect,
        });
        world.insert(ContainerDeco {
            container,
            restriction,
        });
        let local = world.build(ContainerEcho { container });

        let text_rect = world.build(Transform::new(TransformValue::shrink(24, 24), local));
        let debug_text = world.insert(Text {
            text: "Hi there".into(),
            rect: text_rect,
            attr: TextAttr {
                metrics: Metrics::new(12.0, 18.0),
                attrs: Attrs::new().family(Family::Monospace),
                ..Default::default()
            },
            order: 50,
            common,
            modulate: theme.distinct,
            outdated: false,
            canvas_outdated: false,
        });

        world.observer(world.single::<Render>(), move |msg: &String, world| {
            let mut text = world.fetch_mut(debug_text);
            if text.text != *msg {
                text.text.clone_from(msg);
                text.outdated = true;
            }
        });

        world.observer(
            world.single::<LayerPage>(),
            move |LayerDebugMessage(msg), world| {
                let render = world.single_fetch::<Render>();
                if render.timestamp_poll {
                    return;
                }

                let mut text = world.fetch_mut(debug_text);
                if text.text != *msg {
                    text.text.clone_from(msg);
                    text.outdated = true;
                }
            },
        );

        let docker_button = |rect: Handle<Val<IRect>>, image_bytes: &[u8]| {
            world.insert(ToggleButton {
                common: WidgetCommon {
                    camera: common.camera,
                    visible: self.common.visible,
                },
                rect,
                theme: theme.button(),
                image: Some(ButtonImage::new(svg_render(image_bytes), theme.distinct, 8)),
                selected: world.insert(Val::new(false)),
                hovering: world.insert(Val::new(false)),
            })
        };

        let compass_rect = world.build(Transform::new(
            TransformValue::anchor((0.0, 0.0), IRect::new(20, 20, 40, 40)),
            local,
        ));
        let compass = docker_button(
            compass_rect,
            include_bytes!("../../../res/interface/compass.svg"),
        );
        world.observer(compass, move |&ButtonClick, world| {
            let page_camera = world.single_fetch::<PageCamera>().0;
            let utils = world.single_fetch::<PageCameraUtils>().0;
            let mut camera = world.fetch_mut(utils);
            camera.force_clear();
            camera.force_camera_center(I64Vec2::ZERO);
            camera.force_camera_zoom(0);
            camera.write_camera(world, page_camera);
        });

        let page_debug_rect = world.build(Transform::new(
            TransformValue::anchor((0.33, 0.0), IRect::new(-10, 20, 10, 40)),
            local,
        ));
        let page_debug = docker_button(
            page_debug_rect,
            include_bytes!("../../../res/interface/bug.svg"),
        );
        world.observer(page_debug, move |&ButtonSelected(val), world| {
            let mut layer = world.single_fetch_mut::<LayerPage>();
            layer.debug = val;
            world.queue_trigger(page_debug, SetButtonSelected(val));
        });

        let render_profile_rect = world.build(Transform::new(
            TransformValue::anchor((0.67, 0.0), IRect::new(-10, 20, 10, 40)),
            local,
        ));
        let render_profile = docker_button(
            render_profile_rect,
            include_bytes!("../../../res/interface/timer.svg"),
        );
        world.observer(render_profile, move |&ButtonSelected(val), world| {
            let mut render = world.single_fetch_mut::<Render>();
            render.timestamp_poll = val;
            world.queue_trigger(render_profile, SetButtonSelected(val));
        });

        let compact_rect = world.build(Transform::new(
            TransformValue::anchor((1.0, 0.0), IRect::new(-40, 20, -20, 40)),
            local,
        ));
        let compact = docker_button(
            compact_rect,
            include_bytes!("../../../res/interface/database-zap.svg"),
        );
        world.observer(compact, move |&ButtonClick, world| {
            log::debug!("on next startup database will be compacted");
            let sn_file = Save::sn_file(world);
            let sn_tmp_file = Save::sn_tmp_file(world);
            let mut sn_doc = SceneDocument::read(&sn_file);
            sn_doc.set_compact_on_startup(true);
            sn_doc.write(&sn_file, &sn_tmp_file);
        });
    }
}
