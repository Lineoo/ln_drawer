use hashbrown::HashMap;
use winit::event::{TouchPhase, WindowEvent};

use crate::{
    lnwin::Lnwindow,
    measures::{Position, Rectangle, Size},
    render::{canvas::CanvasDescriptor, viewport::Viewport},
    widgets::panel::Panel,
    world::{Element, Handle, World},
};

#[derive(Default)]
pub struct TouchTool {
    touch: HashMap<u64, Handle<Panel>>,
}

impl Element for TouchTool {
    fn when_insert(&mut self, world: &World, _this: Handle<Self>) {
        world.queue(|world| {
            let rect = Rectangle {
                origin: Position::new(0, 0),
                extend: Size::splat(100),
            };

            let bytes = include_bytes!("../../res/icon_hicolor.png");

            world.build(CanvasDescriptor::from_bytes(rect, 0, bytes).unwrap());
        });

        world.observer(
            world.single::<Lnwindow>().unwrap(),
            |event: &WindowEvent, world, lnwindow| {
                let WindowEvent::Touch(touch) = event else {
                    return;
                };

                match touch.phase {
                    TouchPhase::Started => {
                        let lnwindow = world.fetch(lnwindow).unwrap();
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        let location_screen = lnwindow.cursor_to_screen(touch.location);
                        let location_world = viewport.screen_to_world_absolute(location_screen);

                        let mut cache = world.single_fetch_mut::<TouchTool>().unwrap();

                        let panel = world.insert(Panel {
                            rect: Rectangle::new_half(location_world.floor(), Size::new(32, 32)),
                            ..Default::default()
                        });

                        cache.touch.insert(touch.id, panel);
                    }
                    TouchPhase::Moved => {
                        let lnwindow = world.fetch(lnwindow).unwrap();
                        let viewport = world.single_fetch::<Viewport>().unwrap();
                        let location_screen = lnwindow.cursor_to_screen(touch.location);
                        let location_world = viewport.screen_to_world_absolute(location_screen);

                        let cache = world.single_fetch::<TouchTool>().unwrap();

                        let panel = *cache.touch.get(&touch.id).unwrap();
                        let mut panel = world.fetch_mut(panel).unwrap();
                        panel.rect = Rectangle::new_half(location_world.floor(), Size::new(32, 32));
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        let mut cache = world.single_fetch_mut::<TouchTool>().unwrap();

                        let panel = *cache.touch.get(&touch.id).unwrap();
                        world.remove(panel).unwrap();

                        cache.touch.remove(&touch.id);
                    }
                }
            },
        );
    }
}
