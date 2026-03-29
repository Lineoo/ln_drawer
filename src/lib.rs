mod animation;
mod elements;
mod layout;
mod lnwin;
mod measures;
mod render;
mod save;
mod stroke;
mod theme;
mod tools;
mod widgets;
mod world;

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
    use android_logger::{Config, FilterBuilder};
    use winit::{event_loop::EventLoop, platform::android::EventLoopBuilderExtAndroid};

    use crate::world::World;

    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_filter(
                FilterBuilder::new()
                    .filter(None, log::LevelFilter::Debug)
                    .filter(Some("naga"), log::LevelFilter::Warn)
                    .filter(Some("wgpu"), log::LevelFilter::Warn)
                    .build(),
            )
            .with_tag("ln_drawer"),
    );

    log::info!("This is LnDrawer Mobile. Welcome!");

    let mut world = World::default();
    world.insert(app.clone());
    world.flush();
    let lnwin = lnwin::Lnwin {
        world,
        ..Default::default()
    };

    let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();
    event_loop.run_app(lnwin).unwrap();
}
