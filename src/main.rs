#![windows_subsystem = "windows"]
#![allow(dead_code)]

mod elements;
mod interface;
mod lnwin;
mod measures;
mod tools;
pub mod world;
mod text;

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
