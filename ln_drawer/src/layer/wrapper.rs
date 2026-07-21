use std::{sync::mpsc::channel, thread::JoinHandle};

use ln_world::{Element, Handle, World};

use crate::{
    layer::{
        Layer, LayerConfig,
        stream::{self, StreamConfig, ThreadInput, ThreadOutput},
    },
    render::{
        Render, RenderControl, RenderInformation,
        camera::{Camera, CameraBind, CameraPositionChanged},
    },
    save::{Autosave, SaveDatabase},
};

pub struct LayerWrapper {
    pub layer: Layer,
    pub render_debugging: bool,

    thread_tx: std::sync::mpsc::Sender<ThreadInput>,
    thread_rx: std::sync::mpsc::Receiver<ThreadOutput>,
    thread: Option<JoinHandle<()>>,
}

impl LayerWrapper {
    pub fn new(world: &World) -> Self {
        let render = world.single_fetch::<Render>().unwrap();
        let camera_bind = world.single_fetch::<CameraBind>().unwrap();

        let layer = Layer::new(LayerConfig {
            device: render.device.clone(),
            queue: render.queue.clone(),
            surface_format: render.config.format,
            mipmap_levels: 8,
            chunk_size: 512,
            controlled: true,
            camera_bind_layout: camera_bind.layout.clone(),
        });

        let database = world.single_fetch::<SaveDatabase>().unwrap().clone();

        let (input_tx, input_rx) = channel();
        let (output_tx, output_rx) = channel();

        let stream_config = StreamConfig {
            database,
            device: render.device.clone(),
            queue: render.queue.clone(),
            chunk_render_layout: layer.chunk_render_layout.clone(),
            chunk_draw_layout: layer.chunk_draw_layout.clone(),
            chunk_size: layer.chunk_size,
            mipmap_levels: layer.mipmap_levels,
        };

        let camera = world.single_fetch::<Camera>().unwrap();
        input_tx
            .send(ThreadInput::SetStreamCamera(
                camera.zoom,
                camera.size,
                camera.center,
            ))
            .unwrap();

        let thread = std::thread::spawn(move || {
            stream::loading_thread(stream_config, input_rx, output_tx).unwrap();
        });

        LayerWrapper {
            layer,
            render_debugging: false,
            thread_tx: input_tx,
            thread_rx: output_rx,
            thread: Some(thread),
        }
    }

    fn process_stream(&mut self) {
        while let Ok(output) = self.thread_rx.try_recv() {
            match output {
                ThreadOutput::ThreadDebugMessage(_msg) => {
                    log::debug!("layer stream: {_msg}");
                }
                ThreadOutput::Insert(key, chunk_bind) => {
                    self.layer.chunks.insert(key, chunk_bind);
                }
                ThreadOutput::Remove(key) => {
                    self.layer.chunks.remove(&key);
                }
            }
        }
    }
}

impl Drop for LayerWrapper {
    fn drop(&mut self) {
        self.thread_tx.send(ThreadInput::Abort).unwrap();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Element for LayerWrapper {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        world.single::<LayerWrapper>().unwrap();

        let save = world.insert(Autosave(Box::new(move |world, _write| {
            let this = world.single_fetch::<LayerWrapper>().unwrap();
            this.thread_tx.send(ThreadInput::Autosave).unwrap();
        })));

        world.dependency(save, this);

        let camera = world.single::<Camera>().unwrap();
        world.observer(camera, move |_: &CameraPositionChanged, world| {
            let this = world.single_fetch::<LayerWrapper>().unwrap();
            let camera = world.single_fetch::<Camera>().unwrap();

            this.thread_tx
                .send(ThreadInput::SetStreamCamera(
                    camera.zoom,
                    camera.size,
                    camera.center,
                ))
                .unwrap();
        });

        let control = world.insert(RenderControl {
            prepare: Some(Box::new(move |world| {
                let this = &mut *world.fetch_mut(this).unwrap();
                this.process_stream();

                Some(RenderInformation {
                    keep_redrawing: false,
                })
            })),
            draw: Some(Box::new(move |world, rpass| {
                let this = world.single_fetch::<LayerWrapper>().unwrap();
                let camera = world.single_fetch::<Camera>().unwrap();

                this.layer.render(rpass, &camera, this.render_debugging);
            })),
        });

        RenderControl::reorder(Some(-100), world, control);
        world.dependency(control, this);
    }
}
