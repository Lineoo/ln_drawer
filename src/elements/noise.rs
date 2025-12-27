use rodio::{OutputStream, OutputStreamBuilder, Sink, source::noise};

use crate::{
    layout::translatable::TranslatableDescriptor,
    measures::{Position, Rectangle, Size},
    render::canvas::CanvasDescriptor,
    theme::{Attach, Luni},
    widgets::{
        check_button::CheckButtonDescriptor,
        events::{Interact, Switch},
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

        let luni = world.single::<Luni>().unwrap();
        let button = world.build(CheckButtonDescriptor {
            rect,
            checked: false,
            order: 20,
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

        world.queue(move |world| {
            world.trigger(luni, &Attach(button));
        });

        world.observer(button, move |Switch, world, button| {
            let mut button = world.fetch_mut(button).unwrap();
            button.checked = !button.checked;

            let this = world.fetch(this).unwrap();
            match button.checked {
                true => this.sink.play(),
                false => this.sink.pause(),
            }
        });

        world.observer(button, move |interact: &Interact, world, button| {
            if let Interact::PropertyChange = interact {
                let button = world.fetch(button).unwrap();
                let mut icon = world.fetch_mut(icon).unwrap();
                icon.rect = button.rect.expand(-20);
            }
        });

        world.dependency(button, this);
        world.dependency(icon, this);
    }
}
