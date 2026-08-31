use std::sync::Arc;

use cosmic_text::{Attrs, Family, Metrics};
use glam::{I64Vec2, IVec2, UVec2};
use ln_world::{Handle, HandleGeneric, World};

use crate::{
    layer::wrapper::{LayerDebugMessage, LayerWrapper},
    layout::transform::{Transform, TransformValue},
    measures::Rectangle,
    render::{
        Render,
        camera::{CameraUtils, MainCamera},
    },
    save::SaveDatabase,
    theme::Theme,
    widgets::{
        SetWidgetVisible,
        button::{
            ButtonClick, ButtonImage, ButtonSelected, SetButtonSelected, ToggleButton,
            ToggleButtonTheme,
        },
        panel::Panel,
        renderer::{svg::svg_render, text::Text},
    },
};

pub fn debug_panel(world: &World, submenu: Handle<Panel>) {
    let debug_text = world.insert(Text {
        text: "Hi there".into(),
        rect: Rectangle::new_half(IVec2::ZERO, UVec2::ONE),
        metrics: Metrics::new(12.0, 18.0),
        attrs: Attrs::new().family(Family::Monospace),
        order: 50,
        visible: false,
        ..Default::default()
    });

    world.insert(Transform {
        value: TransformValue::shrink(24, 24),
        source: submenu.untyped(),
        target: debug_text.untyped(),
    });

    world.observer(submenu, move |&SetWidgetVisible(visible), world| {
        let mut layer = world.single_fetch_mut::<LayerWrapper>().unwrap();
        layer.debug = visible;
    });

    world.observer(
        world.single::<Render>().unwrap(),
        move |msg: &String, world| {
            let mut text = world.fetch_mut(debug_text).unwrap();
            if text.text != *msg {
                text.text.clone_from(msg);
                text.outdated = true;
            }
        },
    );

    world.observer(
        world.single::<LayerWrapper>().unwrap(),
        move |LayerDebugMessage(msg), world| {
            let render = world.single_fetch::<Render>().unwrap();
            if render.timestamp_poll {
                return;
            }

            let mut text = world.fetch_mut(debug_text).unwrap();
            if text.text != *msg {
                text.text.clone_from(msg);
                text.outdated = true;
            }
        },
    );

    let theme = world.single_fetch::<Theme>().unwrap();
    let docker_button = |image_bytes| {
        world.insert(ToggleButton {
            rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
            theme: ToggleButtonTheme {
                idle_color: theme.primary_color,
                hover_color: theme.secondary_color,
                press_color: theme.highlight_color,
                selected_color: theme.highlight_color,
            },
            image: Some(ButtonImage {
                transform: TransformValue::anchor(
                    (0.5, 0.5),
                    Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
                ),
                bytes: Arc::new(image::DynamicImage::from(svg_render(image_bytes, 1.0))),
            }),
            selected: false,
            visible: true,
            hovering: false,
        })
    };

    let compass = docker_button(include_bytes!("../../../res/interface/compass.svg"));
    world.observer(compass, move |&ButtonClick, world| {
        let main_camera = world.single_fetch::<MainCamera>().unwrap();
        let mut camera = world
            .enter_single_fetch_mut::<CameraUtils>(main_camera.0)
            .unwrap();
        camera.force_clear();
        camera.force_camera_center(I64Vec2::ZERO);
        camera.force_camera_zoom(0);
        world.enter(main_camera.0, || {
            camera.apply_to_camera(world);
        });
    });

    let render_profile = docker_button(include_bytes!("../../../res/interface/timer.svg"));
    world.observer(render_profile, move |&ButtonSelected(val), world| {
        let mut render = world.single_fetch_mut::<Render>().unwrap();
        render.timestamp_poll = val;
        world.queue_trigger(render_profile, SetButtonSelected(val));
    });

    let compact = docker_button(include_bytes!("../../../res/interface/database-zap.svg"));
    world.observer(compact, move |&ButtonClick, world| {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        log::debug!("on next startup database will be compacted");
        SaveDatabase::write_compact(&db.0).unwrap();
    });

    world.insert(Transform {
        value: TransformValue::anchor((0.0, 0.0), Rectangle::new(20, 20, 40, 40)),
        source: submenu.untyped(),
        target: compass.untyped(),
    });
    world.insert(Transform {
        value: TransformValue::anchor((0.5, 0.0), Rectangle::new(-10, 20, 10, 40)),
        source: submenu.untyped(),
        target: render_profile.untyped(),
    });

    world.insert(Transform {
        value: TransformValue::anchor((1.0, 0.0), Rectangle::new(-40, 20, -20, 40)),
        source: submenu.untyped(),
        target: compact.untyped(),
    });
}
