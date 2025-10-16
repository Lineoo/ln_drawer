use std::sync::mpsc::{Receiver, Sender, channel};

use indexmap::IndexMap;
use wgpu::*;

mod painter;
mod square;
mod standard_square;
mod viewport;
mod wireframe;

pub use painter::Painter;
pub use square::Square;
pub use wireframe::Wireframe;
pub use standard_square::StandardSquare;

use crate::{
    interface::{
        painter::PainterBuffer, square::SquareBuffer, standard_square::StandardSquareInstance,
        viewport::InterfaceViewport, wireframe::WireframeBuffer,
    },
    lnwin::Viewport,
    measures::Rectangle,
    world::{Element, InsertElement},
};

/// Main render part
pub struct Interface {
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    device: Device,
    queue: Queue,

    wireframe: wireframe::WireframePipeline,
    square: square::SquarePipeline,
    painter: painter::PainterPipeline,
    standard_square: standard_square::StandardSquareManager,

    components_tx: Sender<(usize, ComponentCommand)>,
    components_rx: Receiver<(usize, ComponentCommand)>,

    components_idx: usize,
    components: IndexMap<usize, Component, hashbrown::DefaultHashBuilder>,

    viewport: InterfaceViewport,
}

impl Element for Interface {}
impl InsertElement for Interface {}

impl Interface {
    pub async fn new(window: impl Into<SurfaceTarget<'static>>, viewport: &Viewport) -> Interface {
        let instance = Instance::default();

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: None,
                required_features: Features::empty(),
                required_limits: Limits::defaults(),
                memory_hints: MemoryHints::MemoryUsage,
                trace: Trace::Off,
            })
            .await
            .unwrap();

        // Surface Configuration
        let surface_config = surface
            .get_default_config(&adapter, viewport.width, viewport.height)
            .unwrap();

        let viewport = InterfaceViewport::new(viewport, &device);

        let standard_square = standard_square::StandardSquareInstance::manager(
            &device,
            &viewport.layout,
            surface_config.format,
        );

        // Render Components
        let wireframe = wireframe::WireframePipeline::init(&device, &surface_config, &viewport);
        let square = square::SquarePipeline::init(&device, &surface_config, &viewport);
        let painter = painter::PainterPipeline::init(&device, &surface_config, &viewport);

        let (components_tx, components_rx) = channel();

        Interface {
            surface,
            surface_config,
            device,
            queue,
            wireframe,
            square,
            painter,
            standard_square,
            components_tx,
            components_rx,
            components_idx: 0,
            components: IndexMap::default(),
            viewport,
        }
    }

    /// Suggested to call before [`Interface::redraw()`]. This will following jobs:
    /// - Remove unattached components
    pub fn restructure(&mut self) {
        for (idx, command) in self.components_rx.try_iter() {
            match command {
                ComponentCommand::Destroy => {
                    self.components.swap_remove(&idx);
                    self.components
                        .sort_by(|_, c1, _, c2| c1.z_order.cmp(&c2.z_order));
                }
                ComponentCommand::SetVisibility(visible) => {
                    if let Some(component) = self.components.get_mut(&idx) {
                        component.visible = visible;
                    }
                }
                ComponentCommand::SetZOrder(z_order) => {
                    if let Some(component) = self.components.get_mut(&idx) {
                        component.z_order = z_order;
                        self.components
                            .sort_by(|_, c1, _, c2| c1.z_order.cmp(&c2.z_order));
                    }
                }
            }
        }
    }

    pub fn redraw(&self) {
        let texture = self.surface.get_current_texture().unwrap();

        let view = texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });

        for component in self.components.values() {
            if !component.visible {
                continue;
            }
            match &component.component {
                ComponentInner::Wireframe(wireframe) => {
                    self.wireframe.set_pipeline(&mut rpass);
                    wireframe.draw(&mut rpass);
                }
                ComponentInner::Square(square) => {
                    self.square.set_pipeline(&mut rpass);
                    square.draw(&mut rpass);
                }
                ComponentInner::Painter(painter) => {
                    self.painter.set_pipeline(&mut rpass);
                    painter.draw(&mut rpass);
                }
                ComponentInner::StandardSquare(standard_square) => {
                    standard_square.draw(&mut rpass, &self.viewport.bind, &self.standard_square);
                }
            }
        }

        drop(rpass);

        self.queue.submit([encoder.finish()]);

        texture.present();
    }

    pub fn resize(&mut self, viewport: &Viewport) {
        self.viewport.resize(viewport, &self.queue);
        self.surface_config.width = viewport.width;
        self.surface_config.height = viewport.height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    #[must_use = "The wireframe will be destroyed when being drop."]
    fn create_wireframe(&mut self, rect: Rectangle, color: [f32; 4]) -> Wireframe {
        let wireframe = (self.wireframe).create(
            rect,
            color,
            self.components_idx,
            self.components_tx.clone(),
            &self.device,
            &self.queue,
        );
        self.insert(Component {
            component: ComponentInner::Wireframe(wireframe.clone_buffer()),
            z_order: 0,
            visible: true,
        });
        wireframe
    }

    #[must_use = "The wireframe will be destroyed when being drop."]
    fn create_square(&mut self, rect: Rectangle, color: [f32; 4]) -> Square {
        let square = (self.square).create(
            rect,
            color,
            self.components_idx,
            self.components_tx.clone(),
            &self.device,
            &self.queue,
        );
        self.insert(Component {
            component: ComponentInner::Square(square.clone_buffer()),
            z_order: 0,
            visible: true,
        });
        square
    }

    fn create_painter(&mut self, rect: Rectangle) -> Painter {
        let empty = vec![0; (rect.width() * rect.height() * 4) as usize];
        let painter = self.painter.create(
            rect,
            empty,
            self.components_idx,
            self.components_tx.clone(),
            &self.device,
            &self.queue,
        );
        self.insert(Component {
            component: ComponentInner::Painter(painter.clone_buffer()),
            z_order: 0,
            visible: true,
        });
        painter
    }

    fn create_painter_with(&mut self, rect: Rectangle, data: Vec<u8>) -> Painter {
        let painter = self.painter.create(
            rect,
            data,
            self.components_idx,
            self.components_tx.clone(),
            &self.device,
            &self.queue,
        );
        self.insert(Component {
            component: ComponentInner::Painter(painter.clone_buffer()),
            z_order: 0,
            visible: true,
        });
        painter
    }

    fn insert(&mut self, component: Component) {
        self.components.insert(self.components_idx, component);
        self.components_idx += 1;
        self.components
            .sort_by(|_, c1, _, c2| c1.z_order.cmp(&c2.z_order));
    }
}

pub struct Redraw;

struct Component {
    component: ComponentInner,
    z_order: isize,
    visible: bool,
}

enum ComponentInner {
    Painter(PainterBuffer),
    Wireframe(WireframeBuffer),
    Square(SquareBuffer),
    StandardSquare(StandardSquareInstance),
}

enum ComponentCommand {
    Destroy,
    SetZOrder(isize),
    SetVisibility(bool),
}
