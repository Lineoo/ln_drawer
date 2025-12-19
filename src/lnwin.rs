use std::{collections::HashSet, sync::Arc};

use sdl3::{
    event::{Event as SdlEvent, WindowEvent as SdlWindowEvent},
    keyboard::Keycode as SdlKeyCode,
    mouse::MouseButton as SdlMouseButton,
    video::Window as SdlWindow,
};
use wgpu::{
    SurfaceTarget,
    rwh::{HasDisplayHandle, HasWindowHandle},
};

use crate::{
    app_runner::{self, AppCtx},
    elements::{image::Image, stroke::StrokeLayer},
    interface::{Interface, Redraw},
    measures::Position,
    text::TextManager,
    tools::{focus::Focus, pointer::Pointer},
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct Lnwin {
    world: World,
}
// impl ApplicationHandler for Lnwin {
//     fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//         if self.world.single::<Lnwindow>().is_none() {
//             let lnwindow = pollster::block_on(Lnwindow::new(event_loop, &mut self.world));
//             self.world.insert(lnwindow);
//             self.world.flush();
//         }
//     }

//     fn window_event(
//         &mut self,
//         event_loop: &ActiveEventLoop,
//         _window_id: WindowId,
//         event: WindowEvent,
//     ) {
//         match self.world.single::<Lnwindow>() {
//             Some(window) => self.world.trigger(window, event),
//             None => event_loop.exit(),
//         }

//         self.world.flush();
//     }

//     fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
//         self.world = World::default();
//     }
// }

impl app_runner::Application for Lnwin {
    fn on_init(&mut self, ctx: &AppCtx) {
        if self.world.single::<Lnwindow>().is_none() {
            let lnwindow =
                pollster::block_on(Lnwindow::new(ctx, &mut self.world, ctx.event.clone()));
            self.world.insert(lnwindow);
            self.world.flush();
        }
    }

    fn on_event(&mut self, event: SdlEvent, ctx: &AppCtx) {
        match self.world.single::<Lnwindow>() {
            Some(window) => self.world.trigger(window, event),
            None => ctx
                .event
                .push_event(SdlEvent::Quit { timestamp: 0 })
                .unwrap(),
        }

        self.world.flush();
    }

    fn on_exit(&mut self, ctx: &AppCtx) {
        self.world = World::default();
    }
}

// contains the unsafe impl as much as possible by putting it in this module
#[derive(Clone)]
struct SyncWindow(SdlWindow);

unsafe impl Send for SyncWindow {}
unsafe impl Sync for SyncWindow {}

impl HasWindowHandle for SyncWindow {
    fn window_handle(&self) -> Result<wgpu::rwh::WindowHandle<'_>, wgpu::rwh::HandleError> {
        self.0.window_handle()
    }
}
impl HasDisplayHandle for SyncWindow {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        self.0.display_handle()
    }
}

/// The main window.
pub struct Lnwindow {
    window: Arc<SyncWindow>,
    viewport: Viewport,

    // Screen-space
    cursor: [f64; 2],

    last_wheel_y: f32,

    camera_cursor_start: [f64; 2],
    camera_origin: Option<[i32; 2]>,

    event: sdl3::EventSubsystem,
}
impl Element for Lnwindow {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        world.observer(this, |event: &SdlEvent, world, this| {
            let mut lnwindow = world.fetch_mut(this).unwrap();
            lnwindow.window_event(event, world, this);
        });

        world.insert(TextManager::default());
        world.insert(LnwinModifiers::default());
        world.insert(Focus::default());
        world.insert(StrokeLayer::default());
        world.insert(Pointer);
    }
}
impl Lnwindow {
    // async fn ne_w(event_loop: &ActiveEventLoop, world: &mut World) -> Lnwindow {
    //     let win_attr = Window::default_attributes();

    //     let window = event_loop.create_window(win_attr).unwrap();
    //     let window = Arc::new(window);

    //     let size = window.inner_size();
    //     let viewport = Viewport {
    //         width: size.width.max(1),
    //         height: size.height.max(1),
    //         camera: [0, 0],
    //         zoom: 0,
    //     };

    //     let interface = Interface::new(window.clone(), &viewport).await;

    //     world.insert(interface);

    //     Lnwindow {
    //         window,
    //         viewport,
    //         cursor: [0.0, 0.0],
    //         camera_cursor_start: [0.0, 0.0],
    //         camera_origin: None,
    //     }
    // }

    async fn new(ctx: &AppCtx, world: &mut World, event: sdl3::EventSubsystem) -> Self {
        let size = (800, 600);
        let window = Arc::new(SyncWindow(
            ctx.video
                .window("LnDrawer", size.0, size.1)
                .build()
                .unwrap(),
        ));

        let viewport = Viewport {
            width: size.0.max(1),
            height: size.1.max(1),
            camera: [0, 0],
            zoom: 0,
        };

        let interface = Interface::new(window.clone(), &viewport).await;

        world.insert(interface);

        Self {
            window,
            viewport,
            cursor: [0.0, 0.0],
            last_wheel_y: 0f32,
            camera_cursor_start: [0.0, 0.0],
            camera_origin: None,
            event,
        }
    }

    fn window_event(&mut self, event: &SdlEvent, world: &World, this: Handle<Lnwindow>) {
        match event {
            SdlEvent::MouseMotion { x, y, .. } => {
                // The viewport needs to be updated before the viewport transform

                self.cursor = self.viewport.cursor_to_screen((*x, *y));
                if let Some(camera_orig) = &mut self.camera_origin {
                    let dx = self.camera_cursor_start[0] - self.cursor[0];
                    let dy = self.camera_cursor_start[1] - self.cursor[1];
                    let [dx, dy] = self.viewport.screen_to_world_relative([dx, dy]);

                    self.viewport.camera = [camera_orig[0] + dx, camera_orig[1] + dy];
                    self.request_redraw();
                }

                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Moved(point));

                self.request_redraw();
            }

            // Major Interaction //
            SdlEvent::MouseButtonDown {
                mouse_btn: SdlMouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Pressed(point));

                self.request_redraw();
            }

            SdlEvent::MouseButtonUp {
                mouse_btn: SdlMouseButton::Left,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerEvent::Released(point));

                self.request_redraw();
            }

            SdlEvent::MouseButtonDown {
                mouse_btn: SdlMouseButton::Right,
                ..
            } => {
                let point = self.viewport.screen_to_world(self.cursor);
                world.trigger(this, PointerAltEvent(point));

                self.request_redraw();
            }

            SdlEvent::KeyDown {
                keycode: Some(keycode),
                ..
            } if is_modifier(*keycode) => {
                let mut fetched = world.single_fetch_mut::<LnwinModifiers>().unwrap();
                fetched.0.insert(*keycode);
            }

            SdlEvent::KeyUp {
                keycode: Some(keycode),
                ..
            } if is_modifier(*keycode) => {
                let mut fetched = world.single_fetch_mut::<LnwinModifiers>().unwrap();
                fetched.0.remove(keycode);
            }

            SdlEvent::KeyDown { .. } => {
                self.request_redraw();
            }

            // Camera Move //
            SdlEvent::MouseButtonDown {
                mouse_btn: SdlMouseButton::Middle,
                ..
            } => {
                self.camera_cursor_start = self.cursor;
                self.camera_origin = Some(self.viewport.camera);
            }

            SdlEvent::MouseButtonUp {
                mouse_btn: SdlMouseButton::Middle,
                ..
            } => {
                self.camera_origin = None;
            }

            SdlEvent::MouseWheel {
                y: delta,
                direction,
                ..
            } => {
                let level = delta.div_euclid(16.0) as i32 + 1;
                self.viewport.zoom += level;
                self.request_redraw();
            }

            // Render //
            SdlEvent::Window {
                win_event: SdlWindowEvent::Exposed,
                ..
            } => {
                let interface = world.single::<Interface>().unwrap();
                let mut fetched = world.fetch_mut(interface).unwrap();
                fetched.resize(&self.viewport);
                world.trigger(world.single::<Interface>().unwrap(), Redraw);
                world.queue(move |world| {
                    let mut fetched = world.fetch_mut(interface).unwrap();
                    fetched.restructure();
                    fetched.redraw();
                });
            }

            SdlEvent::Window {
                win_event: SdlWindowEvent::Resized(w, h),
                ..
            } => {
                self.viewport.width = (*w).max(1) as u32;
                self.viewport.height = (*h).max(1) as u32;
                self.request_redraw();
            }

            SdlEvent::Window {
                win_event: SdlWindowEvent::CloseRequested,
                ..
            } => {
                world.remove(this);
            }

            _ => (),
        }
    }

    fn request_redraw(&self) {
        self.event
            .push_event(SdlEvent::Window {
                timestamp: 0,
                window_id: self.window.0.id(),
                win_event: SdlWindowEvent::Exposed,
            })
            .unwrap();
    }
}

pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub camera: [i32; 2],
    pub zoom: i32,
}
impl Viewport {
    pub fn cursor_to_screen(&self, cursor: (f32, f32)) -> [f64; 2] {
        let x = (cursor.0 * 2.0) as f64 / self.width as f64 - 1.0;
        let y = 1.0 - (cursor.1 * 2.0) as f64 / self.height as f64;
        [x, y]
    }

    pub fn world_to_screen(&self, point: Position) -> [f64; 2] {
        let x = (point.x - self.camera[0]) as f64 / self.width as f64 * 2.0;
        let y = (point.x - self.camera[1]) as f64 / self.height as f64 * 2.0;
        let scale = f64::powi(2.0, self.zoom);
        [x * scale, y * scale]
    }

    pub fn screen_to_world(&self, point: [f64; 2]) -> Position {
        let scale = f64::powi(2.0, self.zoom);
        let x = (point[0] / scale * self.width as f64 / 2.0).floor() as i32 + self.camera[0];
        let y = (point[1] / scale * self.height as f64 / 2.0).floor() as i32 + self.camera[1];
        Position::new(x, y)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> [i32; 2] {
        let scale = f64::powi(2.0, self.zoom);
        let x = (delta[0] / scale * self.width as f64 / 2.0).floor() as i32;
        let y = (delta[1] / scale * self.height as f64 / 2.0).floor() as i32;
        [x, y]
    }
}

/// Pointer that has been transformed into world-space
#[derive(Debug, Clone, Copy)]
pub enum PointerEvent {
    Moved(Position),
    Pressed(Position),
    Released(Position),
}

#[derive(Debug, Clone, Copy)]
pub struct PointerAltEvent(pub Position);

#[derive(Default)]
pub struct LnwinModifiers(pub HashSet<sdl3::keyboard::Keycode>);

impl LnwinModifiers {
    pub fn has_alt(&self) -> bool {
        self.0.contains(&SdlKeyCode::LAlt) || self.0.contains(&SdlKeyCode::RAlt)
    }

    pub fn has_ctrl(&self) -> bool {
        self.0.contains(&SdlKeyCode::LCtrl) || self.0.contains(&SdlKeyCode::RCtrl)
    }

    pub fn has_shift(&self) -> bool {
        self.0.contains(&SdlKeyCode::LShift) || self.0.contains(&SdlKeyCode::RShift)
    }
}

impl Element for LnwinModifiers {}

fn is_modifier(keycode: SdlKeyCode) -> bool {
    match keycode {
        SdlKeyCode::LAlt | SdlKeyCode::RAlt => true,
        SdlKeyCode::LCtrl | SdlKeyCode::RCtrl => true,
        SdlKeyCode::LShift | SdlKeyCode::RShift => true,
        // super key is not supported, but we don't use it anyway
        _ => false,
    }
}
