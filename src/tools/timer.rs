use std::time::{Duration, Instant};

use winit::event::WindowEvent;

use crate::{
    lnwin::Lnwindow,
    world::{Element, Handle, World, WorldError},
};

pub struct Timer {
    pub rest: Duration,
    pub period: Duration,
}

pub struct TimerHit;

impl Timer {
    pub fn new(period: Duration) -> Self {
        Self {
            rest: period,
            period,
        }
    }
}

impl Element for Timer {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        match world.single_fetch_mut::<TimerLastTick>() {
            Ok(mut last_tick) => {
                last_tick.tick(world);
            }
            Err(WorldError::SingletonNoSuch(_)) => {
                world.insert(TimerLastTick(Instant::now()));
            }
            _ => (),
        }
    }
}

struct TimerLastTick(Instant);

impl Element for TimerLastTick {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |_: &WindowEvent, world| {
            let mut this = world.fetch_mut(this).unwrap();
            this.tick(world);
        });
    }
}

impl TimerLastTick {
    fn tick(&mut self, world: &World) {
        let now = Instant::now();
        let delta = now - self.0;

        world.foreach_fetch_mut::<Timer>(|mut timer| {
            timer.rest = timer.rest.saturating_sub(delta);
            if timer.rest.is_zero() {
                timer.rest = timer.period;
                world.trigger(timer.handle(), &TimerHit);
            }
        });

        self.0 = now;
    }
}
