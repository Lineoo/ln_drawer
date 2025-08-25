#![windows_subsystem = "windows"]

#[deprecated]
mod app;
mod layout;
mod interface;
mod lnwin;
mod elements;

use winit::{error::EventLoopError, event_loop::{ControlFlow, EventLoop}};

fn main() -> Result<(), EventLoopError> {
    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = lnwin::Lnwin::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
