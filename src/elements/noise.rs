use rodio::{OutputStream, OutputStreamBuilder, Sink, source::noise};

use crate::{
    layout::translatable::TranslatableDescriptor,
    measures::{Position, Rectangle, Size},
    render::canvas::CanvasDescriptor,
    widgets::{
        button::ButtonDescriptor,
        events::{WidgetClick, WidgetModified},
    },
    world::{Descriptor, Element, Handle, World},
};

pub struct SimpleNoise {
    pub position: Position,
    stream_handle: OutputStream,
    sink: Sink,
}

pub struct SimpleNoiseDescriptor {
    pub position: Position,
}

impl Descriptor for SimpleNoiseDescriptor {
    type Target = Handle<SimpleNoise>;

    fn when_build(self, world: &World) -> Self::Target {
        let stream_handle = OutputStreamBuilder::open_default_stream().unwrap();

        let sink = Sink::connect_new(stream_handle.mixer());
        sink.set_volume(0.7);
        sink.append(noise::Pink::new(44100));
        sink.pause();

        world.insert(SimpleNoise {
            position: self.position,
            stream_handle,
            sink,
        })
    }
}

impl Element for SimpleNoise {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let rect = Rectangle {
            origin: self.position,
            extend: Size::splat(70),
        };

        let button = world.build(ButtonDescriptor {
            rect,
            order: 20,
            theme: None,
        });

        let icon = world.build(
            CanvasDescriptor::from_bytes(
                rect.expand(-20),
                25,
                include_bytes!("../../res/interface/audio.png"),
            )
            .unwrap(),
        );

        world.build(TranslatableDescriptor {
            rect,
            order: 25,
            hollow: true,
            target: button.untyped(),
        });

        world.observer(button, move |WidgetClick, world, button| {
            let button = world.fetch(button).unwrap();

            let play = world.build(ButtonDescriptor {
                rect: button.rect,
                order: 30,
                theme: None,
            });

            let pause = world.build(ButtonDescriptor {
                rect: button.rect.pad_left(10, 1),
                order: 30,
                theme: None,
            });

            world.observer(play, move |WidgetClick, world, play| {
                let this = world.fetch(this).unwrap();
                this.sink.play();
                world.remove(play);
            });

            world.observer(pause, move |WidgetClick, world, pause| {
                let this = world.fetch(this).unwrap();
                this.sink.pause();
                world.remove(pause);
            });

            let button = button.handle();
            world.dependency(play, pause);
            world.dependency(pause, play);
            world.dependency(play, button);
            world.dependency(pause, button);
        });

        world.observer(button, move |WidgetModified, world, button| {
            let button = world.fetch(button).unwrap();
            let mut icon = world.fetch_mut(icon).unwrap();
            icon.rect = button.rect.expand(-20);
        });

        world.dependency(button, this);
        world.dependency(icon, this);
    }
}
