use cosmic_text::{Attrs, Family, Metrics};
use glam::{I64Vec2, IVec2, UVec2};
use ln_world::{Element, Handle, HandleGeneric, World};

use crate::{
    layer::wrapper::{LayerDebugMessage, LayerPage},
    layout::{
        echo::Echo,
        transform::{Transform, TransformValue},
    },
    measures::Rectangle,
    render::{
        Render,
        camera::{Camera, PageCamera, PageCameraUtils},
    },
    theme::Theme,
    widgets::{
        button::{ButtonClick, ButtonSelected, SetButtonSelected, ToggleButton, ToggleButtonTheme},
        common::WidgetCommon,
        container::ContainerWithCamera,
        renderer::{svg::svg_render, text::Text},
    },
};

pub struct DebugPanel {
    pub common: WidgetCommon,
    pub rect: Rectangle,
}

impl Element for DebugPanel {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let (container, camera) = world.build(ContainerWithCamera {
            common: self.common,
            rect: self.rect,
            inner: TransformValue::copy(),
            scroll: I64Vec2::ZERO,
        });

        let debug_text = world.insert(Text {
            text: "Hi there".into(),
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::ONE),
            metrics: Metrics::new(12.0, 18.0),
            attrs: Attrs::new().family(Family::Monospace),
            order: 50,
            visible: false,
            camera: Some(camera),
            ..Default::default()
        });

        Echo::new(world, this).widget_rectangle().widget_visible();

        world.insert(Transform {
            value: TransformValue::copy(),
            source: this.untyped(),
            target: container.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::shrink(24, 24),
            source: container.untyped(),
            target: debug_text.untyped(),
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

        let theme = world.single_fetch::<Theme>();
        let docker_button = smol_butt(world, &theme, camera);

        let compass = docker_button(include_bytes!("../../../res/interface/compass.svg"));
        world.observer(compass, move |&ButtonClick, world| {
            let page_camera = world.single_fetch::<PageCamera>().0;
            let utils = world.single_fetch::<PageCameraUtils>().0;
            let mut camera = world.fetch_mut(utils);
            camera.force_clear();
            camera.force_camera_center(I64Vec2::ZERO);
            camera.force_camera_zoom(0);
            camera.apply_to_camera(world, page_camera);
        });

        let page_debug = docker_button(include_bytes!("../../../res/interface/bug.svg"));
        world.observer(page_debug, move |&ButtonSelected(val), world| {
            let mut layer = world.single_fetch_mut::<LayerPage>();
            layer.debug = val;
            world.queue_trigger(page_debug, SetButtonSelected(val));
        });

        let render_profile = docker_button(include_bytes!("../../../res/interface/timer.svg"));
        world.observer(render_profile, move |&ButtonSelected(val), world| {
            let mut render = world.single_fetch_mut::<Render>();
            render.timestamp_poll = val;
            world.queue_trigger(render_profile, SetButtonSelected(val));
        });

        let compact = docker_button(include_bytes!("../../../res/interface/database-zap.svg"));
        world.observer(compact, move |&ButtonClick, _world| {
            log::warn!("cannot compact on wasm32");
        });

        world.insert(Transform {
            value: TransformValue::anchor((0.0, 0.0), Rectangle::new(20, 20, 40, 40)),
            source: container.untyped(),
            target: compass.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor((0.33, 0.0), Rectangle::new(-10, 20, 10, 40)),
            source: container.untyped(),
            target: page_debug.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor((0.67, 0.0), Rectangle::new(-10, 20, 10, 40)),
            source: container.untyped(),
            target: render_profile.untyped(),
        });

        world.insert(Transform {
            value: TransformValue::anchor((1.0, 0.0), Rectangle::new(-40, 20, -20, 40)),
            source: container.untyped(),
            target: compact.untyped(),
        });

        world.dependency(debug_text, this);
        world.dependency(render_profile, this);
        world.dependency(compass, this);
        world.dependency(compact, this);
        world.dependency(container, this);
        world.dependency(camera, this);
    }
}

pub fn smol_butt(
    world: &World,
    theme: &Theme,
    camera: Handle<Camera>,
) -> impl Fn(&[u8]) -> Handle<ToggleButton> {
    move |image_bytes| {
        world.insert(ToggleButton {
            common: WidgetCommon {
                camera,
                visible: false,
            },
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(16)),
            theme: ToggleButtonTheme {
                idle_color: theme.primary_color,
                hover_color: theme.secondary_color,
                press_color: theme.highlight_color,
                selected_color: theme.highlight_color,
            },
            image: Some(svg_render(image_bytes, 8)),
            selected: false,
            hovering: false,
        })
    }
}
