use std::{sync::Arc, time::Duration};

use cosmic_text::Metrics;
use glam::{I64Vec2, IVec2, UVec2, Vec2};
use hashbrown::HashMap;
use ln_world::{Element, Handle, ViewOptions, World};
use palette::{Hsla, IntoColor, RgbHue, Srgba};
#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    layout::{
        luni::{LuniAlign, LuniAxis, LuniChild, LuniChildTemplate, LuniFlex, LuniParent, LuniRect},
        transform::{Transform, TransformEdge, TransformValue},
    },
    measures::{FI64Ext, Rectangle},
    render::{
        Render,
        camera::{Camera, CameraDescriptor, CameraUtils, MainCamera, UICamera},
        canvas::Canvas,
        rectangle::RectangleMesh,
        rounded::RoundedRect,
        text::{Text, TextChanged},
    },
    save::{Autosave, AutosaveScheduler, SaveDatabase},
    stroke::{StrokeLayer, StrokeLayerDebugMessage, modifier::Modifier},
    theme::Theme,
    tools::{
        collider::ToolColliderDispatcher, focus::Focus, modifiers::ModifiersTool, mouse::MouseTool,
        pointer::PointerTool, touch::MultiTouchTool,
    },
    widgets::{
        WidgetClick, WidgetEnabled, WidgetHsla, WidgetRectangle,
        button::{Button, ButtonAnim, ButtonChecked, ButtonColor, ButtonImage},
        palette::hsl::{PaletteHsl, PaletteHslMaterial},
        panel::{Panel, PanelAnimation},
        renderer::grid::{Grid, GridMaterial},
        slider::{SetSlider, VSlider},
    },
};

#[derive(Default)]
pub struct Lnwin {
    pub world: World,
    pub windows: HashMap<WindowId, Handle>,
}

impl ApplicationHandler for Lnwin {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.windows.is_empty() {
            let lnwindow = Lnwindow::new(event_loop);
            let root = self.world.here();
            let window_id = lnwindow.window.id();
            let lnwindow = self.world.insert(lnwindow);
            self.windows.insert(window_id, lnwindow.untyped());
            self.world.enter(lnwindow, || {
                self.world.option(ViewOptions { refs: vec![root] });
            });
        } else {
            for &view in self.windows.values() {
                self.world.enter(view, || {
                    let mut render = self.world.single_fetch_mut::<Render>().unwrap();
                    let lnwindow = self.world.single_fetch::<Lnwindow>().unwrap();
                    render.surface_recreate(&lnwindow);
                });
            }
        }

        self.world.flush();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(&view) = self.windows.get(&window_id) {
            self.world.enter(view, || {
                if let Ok(lnwindow) = self.world.single::<Lnwindow>() {
                    self.world.trigger(lnwindow, &event);
                } else {
                    self.windows.remove(&window_id);
                }
            });

            self.world.flush();
        }

        if self.windows.is_empty() {
            event_loop.exit()
        }
    }

    fn suspended(&mut self, _event_loop: &dyn ActiveEventLoop) {
        for &view in self.windows.values() {
            self.world.enter(view, || {
                Autosave::autosave_all(&self.world);
            });
        }
    }
}

/// The main window.
pub struct Lnwindow {
    pub window: Arc<dyn Window>,
}

impl Element for Lnwindow {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, move |event: &WindowEvent, world| {
            if let WindowEvent::CloseRequested = event {
                Autosave::autosave_all(world);
                world.queue(|world| {
                    world.clear();
                });
            }
        });

        world.queue(move |world| {
            let lnwindow = world.fetch_mut(this).unwrap();
            world.insert(pollster::block_on(Render::new(&lnwindow)));
        });

        world.queue(|world| {
            SaveDatabase::init(world);
            world.insert(AutosaveScheduler {
                autosave_duration: Duration::from_secs(180),
            });
        });

        world.queue(|world| {
            Camera::init(world);
            world.flush();

            Canvas::init(world);
            Text::init(world);
            RoundedRect::init(world);
            RectangleMesh::<PaletteHslMaterial>::init(world);
            RectangleMesh::<GridMaterial>::init(world);
            world.insert(Theme::default());
        });

        world.queue(|world| {
            world.insert(ToolColliderDispatcher);
            world.insert(PointerTool::default());
            world.insert(MouseTool::default());
            world.insert(MultiTouchTool::default());
            world.insert(Focus::default());
            world.insert(ModifiersTool::default());
        });

        world.queue(|world| {
            let here = world.here();

            let camera1 = Camera::build_from_save(world, "camera1");
            world.insert(MainCamera(camera1));

            let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
            let size = lnwindow.window.surface_size();
            let camera2 = world.build(CameraDescriptor {
                size: UVec2::new(size.width, size.height),
                zoom: i64::q32_from_f64(lnwindow.window.scale_factor().log2()),
                ..Default::default()
            });
            world.insert(UICamera(camera2));
            drop(lnwindow);

            world.flush();

            world.enter(camera1, || {
                world.option(ViewOptions { refs: vec![here] });
            });
            world.enter(camera2, || {
                world.option(ViewOptions { refs: vec![here] });
            });

            world.flush();
            world.enter(camera1, || {
                world.queue(|world| {
                    world.insert(StrokeLayer::new(world));
                    world.insert(Grid);
                    world.insert(CameraUtils::default());
                });
            });

            world.flush();
            world.enter(camera2, || {
                let stroke = world.enter(camera1, || world.single::<StrokeLayer>().unwrap());
                world.option(ViewOptions {
                    refs: vec![here, stroke.untyped()],
                });
                world.queue(move |world| {
                    world.insert(CameraUtils::default());
                    let lnwindow = world.single::<Lnwindow>().unwrap();
                    world.observer(lnwindow, move |event: &WindowEvent, world| {
                        if let WindowEvent::SurfaceResized(size) = event {
                            let lnwindow = world.fetch(lnwindow).unwrap();
                            let mut camera2 = world.fetch_mut(camera2).unwrap();

                            let scale = lnwindow.window.scale_factor();
                            world.queue_trigger(
                                lnwindow.handle(),
                                WidgetRectangle(Rectangle::new_half(
                                    IVec2::ZERO,
                                    (UVec2::new(size.width / 2, size.height / 2).as_dvec2()
                                        / scale)
                                        .round()
                                        .as_uvec2(),
                                )),
                            );

                            camera2.zoom = i64::q32_from_f64(scale.log2());
                        }
                    });
                });

                world.queue(side_panel);
            });
        });
    }
}

fn side_panel(world: &mut World) {
    let lnwindow = world.single::<Lnwindow>().unwrap();
    let theme = world.single_fetch::<Theme>().unwrap();

    let side_panel = world.insert(Button {
        order: 0,
        color: theme.primary_color,
        active_color: theme.primary_color,
        press_color: theme.primary_color,
        roundness: theme.roundness,
        ..Default::default()
    });

    let pen = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: Some(ButtonImage {
            transform: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
            ),
            bytes: include_bytes!("../res/interface/pen.png"),
        }),
        ..Default::default()
    });

    let brush = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: Some(ButtonImage {
            transform: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
            ),
            bytes: include_bytes!("../res/interface/brush.png"),
        }),
        ..Default::default()
    });

    let eraser = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: Some(ButtonImage {
            transform: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
            ),
            bytes: include_bytes!("../res/interface/eraser.png"),
        }),
        ..Default::default()
    });

    let color_picker = color_palette(world, &theme);

    let elastic_blank = world.insert(());

    let slider = world.insert(VSlider {
        x: 0,
        y_min: -100,
        y_max: 100,
        min: 0.0,
        max: 100.0,
        value: 67.0,
    });

    world.queue(move |world| {
        VSlider::receive_event(slider, world);
        VSlider::create_renderer(slider, world);
        VSlider::create_interact(slider, world);
    });

    world.observer(pen, move |&WidgetClick, world| {
        world.trigger(pen, &ButtonChecked(true));
        world.trigger(brush, &ButtonChecked(false));
        world.trigger(eraser, &ButtonChecked(false));
        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
        stroke.modifier = Modifier {
            min_size: 0.0,
            max_size: 6.0,
            size_force_exp: 1.0,
            min_flow: 0.7,
            max_flow: 1.0,
            flow_force_exp: 2.0,
            softness: 0.2,
            ..stroke.modifier
        };
        stroke.erase = false;

        let slider = world.fetch(slider).unwrap();
        world.queue_trigger(
            slider.handle(),
            SetSlider {
                max: slider.max,
                min: slider.min,
                value: (6.0f32 / 40.0 + 1.0).log2() * (slider.max - slider.min) + slider.min,
            },
        );
    });

    world.observer(brush, move |&WidgetClick, world| {
        world.trigger(pen, &ButtonChecked(false));
        world.trigger(brush, &ButtonChecked(true));
        world.trigger(eraser, &ButtonChecked(false));
        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
        stroke.modifier = Modifier {
            min_size: 1.0,
            max_size: 25.0,
            size_force_exp: 1.0,
            min_flow: 0.1,
            max_flow: 1.0,
            flow_force_exp: 1.0,
            softness: 0.5,
            ..stroke.modifier
        };
        stroke.erase = false;

        let slider = world.fetch(slider).unwrap();
        world.queue_trigger(
            slider.handle(),
            SetSlider {
                max: slider.max,
                min: slider.min,
                value: (24.0f32 / 40.0 + 1.0).log2() * (slider.max - slider.min) + slider.min,
            },
        );
    });

    world.observer(eraser, move |&WidgetClick, world| {
        world.trigger(pen, &ButtonChecked(false));
        world.trigger(brush, &ButtonChecked(false));
        world.trigger(eraser, &ButtonChecked(true));
        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
        stroke.modifier = Modifier {
            min_size: 10.0,
            max_size: 50.0,
            size_force_exp: 1.0,
            min_flow: 0.1,
            max_flow: 1.0,
            flow_force_exp: 1.0,
            softness: 0.5,
            ..stroke.modifier
        };
        stroke.erase = true;

        let slider = world.fetch(slider).unwrap();
        world.queue_trigger(
            slider.handle(),
            SetSlider {
                max: slider.max,
                min: slider.min,
                value: (40.0f32 / 40.0 + 1.0).log2() * (slider.max - slider.min) + slider.min,
            },
        );
    });

    world.observer(slider, move |&SetSlider { min, max, value }, world| {
        let mut stroke = world.single_fetch_mut::<StrokeLayer>().unwrap();
        let percent = (value - min) / (max - min);
        stroke.modifier = Modifier {
            max_size: stroke.modifier.min_size + (percent.exp2() - 1.0) * 40.0,
            ..stroke.modifier
        };
    });

    let compass = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: Some(ButtonImage {
            transform: TransformValue::anchor(
                (0.5, 0.5),
                Rectangle::new_half(IVec2::ZERO, UVec2::splat(8)),
            ),
            bytes: include_bytes!("../res/interface/compass.png"),
        }),
        ..Default::default()
    });

    world.observer(compass, move |&WidgetClick, world| {
        let main_camera = world.single_fetch::<MainCamera>().unwrap();
        let mut camera = world
            .enter_single_fetch_mut::<Camera>(main_camera.0)
            .unwrap();
        camera.center = I64Vec2::ZERO;
    });

    let debug = debug_panel(world, &theme);

    world.insert(Transform {
        value: TransformValue {
            left: TransformEdge {
                anchor: 0.0,
                offset: 24,
            },
            down: TransformEdge {
                anchor: 0.5,
                offset: -240,
            },
            right: TransformEdge {
                anchor: 0.0,
                offset: 24 + 44,
            },
            up: TransformEdge {
                anchor: 0.5,
                offset: 240,
            },
        },
        source: lnwindow.untyped(),
        target: side_panel.untyped(),
    });

    world.insert(LuniFlex {
        parent: (
            side_panel.untyped(),
            LuniParent {
                axis: LuniAxis::Column,
                template: LuniChildTemplate {
                    basis: 36,
                    cross: 36,
                    align: LuniAlign::Center,
                    ..Default::default()
                },
                padding: LuniRect {
                    left: 0,
                    bottom: 4,
                    right: 0,
                    top: 4,
                },
                gap: 4,
                ..Default::default()
            },
        ),
        children: vec![
            (pen.untyped(), LuniChild::default()),
            (brush.untyped(), LuniChild::default()),
            (eraser.untyped(), LuniChild::default()),
            (color_picker.untyped(), LuniChild::default()),
            (
                elastic_blank.untyped(),
                LuniChild {
                    basis: Some(0),
                    grow: Some(1.0),
                    ..Default::default()
                },
            ),
            (
                slider.untyped(),
                LuniChild {
                    basis: Some(160),
                    shrink: Some(1.0),
                    margin: Some(LuniRect {
                        left: 0,
                        bottom: 5,
                        right: 0,
                        top: 5,
                    }),
                    ..Default::default()
                },
            ),
            (compass.untyped(), LuniChild::default()),
            (debug.untyped(), LuniChild::default()),
        ],
    });

    world.queue_trigger(side_panel, WidgetRectangle(Rectangle::new(0, 0, 500, 100)));
}

fn color_palette(world: &World, theme: &Theme) -> Handle<Button> {
    let color_picker = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: None,
        ..Default::default()
    });

    let color_picker_color = world.insert(Button {
        order: 11,
        color: Srgba::new(0.9, 0.7, 0.7, 1.0),
        attach_pointer: false,
        roundness: 10.0,
        shadow_offset: Vec2::ZERO,
        ..Default::default()
    });

    let main_panel_transform = TransformValue::anchor(
        (1.0, 0.5),
        Rectangle::new_half(IVec2::new(144 + 20, 0), UVec2::splat(144)),
    );

    let main_panel_transform_start = TransformValue::anchor(
        (1.0, 0.5),
        Rectangle::new_half(IVec2::new(144 / 4 + 20, 0), UVec2::splat(144 / 4)),
    );

    let palette_transform = TransformValue::scale(0.8, 0.8);

    let main_panel = world.insert(Button {
        attach_pointer: false,
        order: 0,
        enabled: false,
        ..Default::default()
    });

    let palette = world.insert(PaletteHsl {
        rect: Rectangle::default(),
        color: Hsla::new(RgbHue::from_degrees(0.3), 0.5, 0.5, 1.0),
        enabled: false,
    });

    world.dependency(palette, main_panel);

    world.insert(Transform {
        value: main_panel_transform,
        source: color_picker.untyped(),
        target: main_panel.untyped(),
    });

    world.insert(Transform {
        value: palette_transform,
        source: main_panel.untyped(),
        target: palette.untyped(),
    });

    world.observer(palette, move |&WidgetHsla(color), world| {
        let mut layer = world.single_fetch_mut::<StrokeLayer>().unwrap();
        layer.modifier.color = color.into_color();
        world.queue_trigger(color_picker_color, ButtonColor(color.into_color()));
    });

    world.observer(color_picker, move |&WidgetClick, world| {
        let main_panel = world.fetch(main_panel).unwrap();
        let child2 = world.fetch(color_picker).unwrap();
        world.queue_trigger(main_panel.handle(), WidgetEnabled(!main_panel.enabled));
        world.queue_trigger(palette, WidgetEnabled(!main_panel.enabled));

        if !main_panel.enabled {
            world.queue_trigger(
                main_panel.handle(),
                ButtonAnim {
                    src: main_panel_transform_start.compute(child2.rect),
                    dst: main_panel_transform.compute(child2.rect),
                    hidden_after_finished: false,
                },
            );
        }
    });

    world.insert(Transform {
        value: TransformValue::anchor(
            (0.5, 0.5),
            Rectangle::new_half(IVec2::ZERO, UVec2::splat(10)),
        ),
        source: color_picker.untyped(),
        target: color_picker_color.untyped(),
    });
    color_picker
}

fn debug_panel(world: &World, theme: &Theme) -> Handle<Button> {
    let button = world.insert(Button {
        order: 10,
        color: theme.primary_color,
        active_color: theme.secondary_color,
        press_color: theme.highlight_color,
        shadow_color: Srgba::new(0.0, 0.0, 0.0, 0.0),
        roundness: theme.roundness,
        image: None,
        ..Default::default()
    });

    let submenu = world.insert(Panel {
        rect: Rectangle::default(),
        visible: false,
    });

    let debug_text = world.insert(Text {
        text: "Hi there".into(),
        rect: Rectangle::new_half(IVec2::ZERO, UVec2::splat(120)),
        metrics: Metrics::new(12.0, 18.0),
        color: Srgba::new(0, 0, 0, 1),
        upscale: 2.0,
        order: 1,
        visible: false,
    });

    world.queue(move |world| {
        Panel::receive_event(submenu, world);
        Panel::create_renderer(submenu, world);
        Panel::create_interact(submenu, world);
        world
            .fetch_mut(debug_text)
            .unwrap()
            .bind_render(world, debug_text);
    });

    let submenu_transform = TransformValue::anchor(
        (1.0, 0.0),
        Rectangle::new_half(IVec2::new(144 + 20, 144), UVec2::splat(144)),
    );

    let submenu_transform_start = TransformValue::anchor(
        (1.0, 0.0),
        Rectangle::new_half(IVec2::new(144 / 4 + 20, 144 / 4), UVec2::splat(144 / 4)),
    );

    world.insert(Transform {
        value: submenu_transform,
        source: button.untyped(),
        target: submenu.untyped(),
    });

    world.insert(Transform {
        value: TransformValue::shrink(24, 24),
        source: submenu.untyped(),
        target: debug_text.untyped(),
    });

    world.observer(button, move |&WidgetClick, world| {
        let submenu = world.fetch(submenu).unwrap();
        let child2 = world.fetch(button).unwrap();
        world.queue_trigger(submenu.handle(), WidgetEnabled(!submenu.visible));
        world.queue_trigger(debug_text, WidgetEnabled(!submenu.visible));

        if !submenu.visible {
            world.queue_trigger(
                submenu.handle(),
                PanelAnimation {
                    src: submenu_transform_start.compute(child2.rect),
                    dst: submenu_transform.compute(child2.rect),
                    hidden_after_finished: false,
                },
            );
        }
    });

    world.observer(
        world.single::<StrokeLayer>().unwrap(),
        move |StrokeLayerDebugMessage(msg), world| {
            let mut text = world.fetch_mut(debug_text).unwrap();
            text.text.clone_from(msg);
            world.queue_trigger(debug_text, TextChanged);
        },
    );

    button
}

impl Lnwindow {
    fn new(event_loop: &dyn ActiveEventLoop) -> Lnwindow {
        let win_attr = WindowAttributes::default()
            .with_transparent(true)
            .with_title("LnDrawer");

        let window = event_loop.create_window(win_attr).unwrap();
        let window = Arc::from(window);

        Lnwindow { window }
    }

    pub fn cursor_to_screen(&self, position: PhysicalPosition<f64>) -> [f64; 2] {
        let size = self.window.surface_size();
        let x = (position.x * 2.0) / size.width as f64 - 1.0;
        let y = 1.0 - (position.y * 2.0) / size.height as f64;
        [x, y]
    }
}

#[cfg(target_os = "android")]
pub struct LnAndroid(pub AndroidApp);

#[cfg(target_os = "android")]
impl Element for LnAndroid {}
