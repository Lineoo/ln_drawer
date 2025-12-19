use sdl3::{event::Event as SdlEvent, keyboard::Keycode};

use crate::{
    lnwin::Lnwindow,
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct Focus {
    focus: Option<Handle>,
}

impl Element for Focus {
    fn when_inserted(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();

        world.observer(lnwindow, move |event: &SdlEvent, world, _| {
            let keycode;
            let state;
            match event {
                SdlEvent::KeyDown { keycode: kc, .. } => {
                    keycode = kc.unwrap();
                    state = FocusInputKeyState::Press
                }
                SdlEvent::KeyUp { keycode: kc, .. } => {
                    keycode = kc.unwrap();
                    state = FocusInputKeyState::Release
                }
                _ => {
                    return;
                }
            }

            let fetched = world.fetch(this).unwrap();

            if let Some(focus_on) = fetched.focus {
                world.trigger(focus_on, FocusInput { keycode, state });
            }
        });

        world.observer(this, |&RequestFocus(on), world, this| {
            let mut fetched = world.fetch_mut(this).unwrap();

            let off = fetched.focus;
            fetched.focus = on;

            if off != on {
                if let Some(off) = off {
                    world.trigger(off, FocusLeave);
                }

                if let Some(on) = on {
                    world.trigger(on, FocusEnter);
                }
            }
        });
    }
}

pub struct RequestFocus(pub Option<Handle>);

pub struct FocusEnter;

pub struct FocusLeave;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusInputKeyState {
    Press,
    Release,
}

pub struct FocusInput {
    pub keycode: Keycode,
    pub state: FocusInputKeyState,
}
