#[cfg(target_os = "android")]
mod android;
mod animation;
mod color;
mod i18n;
mod layer;
mod layout;
mod lnwin;
mod measures;
mod render;
mod save;
mod theme;
mod tools;
mod widgets;

pub fn desktop_main() {
    use winit::event_loop::EventLoop;

    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let lnwin = lnwin::Lnwin::default();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.run_app(lnwin).unwrap();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use ln_world::World;
    use winit::{
        event_loop::EventLoop,
        platform::android::{EventLoopBuilderExtAndroid, activity::WindowManagerFlags},
    };

    android::init_logging();

    log::info!("This is LnDrawer Mobile. Welcome!");

    app.set_window_flags(WindowManagerFlags::FULLSCREEN, WindowManagerFlags::empty());

    android::notify_java(&app, "android_main started");

    let mut world = World::default();
    world.insert(lnwin::LnAndroid(app.clone()));
    world.flush();
    let lnwin = lnwin::Lnwin {
        world,
        ..Default::default()
    };

    let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();
    event_loop.run_app(lnwin).unwrap();
}
