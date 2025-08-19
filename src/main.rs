mod app;
mod renderer;
mod layout;

use winit::{error::EventLoopError, event_loop::{ControlFlow, EventLoop}};

use crate::app::LnDrawer;

fn main() -> Result<(), EventLoopError> {
    env_logger::init();

    log::info!("This is LnDrawer. Welcome!");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = LnDrawer::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}
