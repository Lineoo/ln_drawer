#![windows_subsystem = "windows"]

mod elements;
mod lnwin;
mod measures;
mod render;
mod save;
mod tools;
mod world;

use winit::{
    error::EventLoopError,
    event_loop::{ControlFlow, EventLoop},
};

fn main() -> Result<(), EventLoopError> {
    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = lnwin::Lnwin::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
