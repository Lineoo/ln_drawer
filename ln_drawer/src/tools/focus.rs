use ln_world::{Element, Handle, HandleAny, World};
use winit::event::{KeyEvent, WindowEvent};

use crate::lnwin::Lnwindow;

#[derive(Default)]
pub struct FocusTool {
    focus: Option<HandleAny>,
}

impl Element for FocusTool {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();

        world.observer(lnwindow, move |event: &WindowEvent, world| {
            let WindowEvent::KeyboardInput { event, .. } = event else {
                return;
            };

            let fetched = world.fetch(this);

            if let Some(focus_on) = fetched.focus {
                world.trigger(focus_on, &FocusInput(event.clone()));
            }
        });

        world.observer(this, move |&RequestFocus(on), world| {
            let mut fetched = world.fetch_mut(this);

            let off = fetched.focus;
            fetched.focus = on;

            if off != on {
                if let Some(off) = off {
                    world.trigger(off, &FocusLeave);
                }

                if let Some(on) = on {
                    world.trigger(on, &FocusEnter);
                }
            }
        });
    }
}

pub struct RequestFocus(pub Option<HandleAny>);

pub struct FocusEnter;

pub struct FocusLeave;

pub struct FocusInput(pub KeyEvent);
