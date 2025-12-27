#![windows_subsystem = "windows"]

mod elements;
mod lnwin;
mod measures;
mod render;
mod save;
mod theme;
mod tools;
mod widgets;
mod world;
mod animation;

use winit::{error::EventLoopError, event_loop::EventLoop};

fn main() -> Result<(), EventLoopError> {
    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let event_loop = EventLoop::new()?;
    let mut app = lnwin::Lnwin::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
