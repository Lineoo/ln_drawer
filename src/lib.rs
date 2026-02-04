mod animation;
mod elements;
mod layout;
mod lnwin;
mod measures;
mod render;
mod save;
mod theme;
mod tools;
mod widgets;
mod world;

pub fn desktop_main() {
    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let mut app = lnwin::Lnwin::default();
    event_loop.run_app(&mut app).unwrap();
}
