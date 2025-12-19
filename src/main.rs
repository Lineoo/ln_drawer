#![windows_subsystem = "windows"]

mod app_runner;
mod elements;
mod interface;
mod lnwin;
mod measures;
mod save;
mod text;
mod tools;
mod world;

use crate::app_runner::run_app;

// fn main() -> Result<(), EventLoopError> {
//     env_logger::init();

//     log::info!("This is LnDrawer. Welcome!");

//     let event_loop = EventLoop::new()?;
//     event_loop.set_control_flow(ControlFlow::Wait);
//     let mut app = lnwin::Lnwin::default();
//     event_loop.run_app(&mut app)?;
//     Ok(())
// }

fn main() -> Result<(), sdl3::Error> {
    let app = lnwin::Lnwin::default();
    run_app(app)
}
