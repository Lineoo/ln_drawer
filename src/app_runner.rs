use sdl3::event::Event;

pub(crate) trait Application {
    fn on_init(&mut self, ctx: &AppCtx);
    fn on_event(&mut self, event: Event, ctx: &AppCtx);
    fn on_exit(&mut self, ctx: &AppCtx);
}

pub(crate) struct AppCtx {
    pub sdl: sdl3::Sdl,
    pub video: sdl3::VideoSubsystem,
    pub event: sdl3::EventSubsystem,
}

pub(crate) fn run_app<App: Application>(mut app: App) -> Result<(), sdl3::Error> {
    let sdl = sdl3::init()?;
    let video = sdl.video()?;
    let event = sdl.event()?;
    let ctx = AppCtx { sdl, video, event };
    let mut event_pump = ctx.sdl.event_pump()?;
    app.on_init(&ctx);
    'event_loop: loop {
        for event in event_pump.poll_iter() {
            if let Event::Quit { .. } = event {
                break 'event_loop;
            }
            app.on_event(event, &ctx);
        }
    }
    app.on_exit(&ctx);
    Ok(())
}
