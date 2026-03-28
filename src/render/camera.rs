use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;
use winit::event::WindowEvent;

use crate::lnwin::Lnwindow;
use crate::measures::{Fract, PositionFract, Size};
use crate::render::{Render, RenderControl};
use crate::save::{SaveControl, SaveControlRead, SaveControlWrite};
use crate::world::{Descriptor, Element, Handle, ViewId, World, WorldError};

pub struct Camera {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,

    pub bind: BindGroup,
    pub uniform: Buffer,
    pub layout: BindGroupLayout,

    queue: Queue,
    control: Handle<RenderControl>,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CameraDescriptor {
    pub size: Size,
    pub center: PositionFract,
    pub zoom: Fract,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    size: [u32; 2],
    center: [i32; 2],
    center_fract: [u32; 2],
    zoom: i32,
    zoom_fract: u32,
}

impl Descriptor for CameraDescriptor {
    type Target = Handle<Camera>;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>().unwrap();

        let layout = render
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("camera_bind_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform = render.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("camera_uniform"),
            contents: bytemuck::bytes_of(&CameraUniform {
                size: self.size.into_array(),
                center: self.center.into_array(),
                center_fract: self.center.into_arrayf(),
                zoom: self.zoom.n,
                zoom_fract: self.zoom.nf,
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = render.device.create_bind_group(&BindGroupDescriptor {
            label: Some("camera_bind"),
            layout: &layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let control = world.insert(RenderControl {
            visible: true,
            order: 0,
            refreshing: false,
            prepare: None,
            draw: None,
        });

        world.insert(Camera {
            size: self.size,
            center: self.center,
            zoom: self.zoom,
            uniform,
            bind,
            layout,
            queue: render.queue.clone(),
            control,
        })
    }
}

impl Element for Camera {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            if let WindowEvent::SurfaceResized(size) = event {
                let mut camera = world.fetch_mut(this).unwrap();

                camera.size.w = size.width;
                camera.size.h = size.height;
            }
        });
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&CameraUniform {
                size: Size::new(self.size.w.max(1), self.size.h.max(1)).into_array(),
                center: self.center.into_array(),
                center_fract: self.center.into_arrayf(),
                zoom: self.zoom.n,
                zoom_fract: self.zoom.nf,
            }),
        );

        world.fetch_mut(self.control).unwrap().modified();
    }
}

impl Camera {
    #[inline]
    pub fn screen_to_world_absolute(&self, point: [f64; 2]) -> PositionFract {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> PositionFract {
        let scale = (self.zoom.n as f64 + self.zoom.nf as f64 * (-32f64).exp2()).exp2();
        let x = delta[0] / scale * self.size.w as f64 / 2.0;
        let y = delta[1] / scale * self.size.h as f64 / 2.0;
        PositionFract::new(Fract::from_f64(x), Fract::from_f64(y))
    }

    pub fn world_to_screen_absolute(&self, point: PositionFract) -> [f64; 2] {
        self.world_to_screen_relative(point - self.center)
    }

    pub fn world_to_screen_relative(&self, point: PositionFract) -> [f64; 2] {
        let scale = (self.zoom.n as f64 + self.zoom.nf as f64 * (-32f64).exp2()).exp2();
        let x = point.x.into_f64() * 2.0 / self.size.w as f64 * scale;
        let y = point.y.into_f64() * 2.0 / self.size.h as f64 * scale;
        [x, y]
    }

    pub fn init(world: &mut World, name: &str) {
        world.insert(Camera::save_read(world, name));
        world.flush();

        if let Err(WorldError::SingletonNoSuch(_)) = world.single::<Camera>() {
            Camera::build_default(world, name);
        }
    }

    fn build_default(world: &World, name: &str) {
        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        let size = lnwindow.window.surface_size();

        let camera = world.build(CameraDescriptor {
            size: Size::new(size.width, size.height),
            ..Default::default()
        });

        let control = SaveControl::create(name.into(), world, &[]);
        world.insert(Camera::save_write(camera, control));
    }

    fn save_read(world: &World, name: &str) -> SaveControlRead {
        SaveControlRead {
            name: name.into(),
            read: Box::new(move |world, control| {
                let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
                let size = lnwindow.window.surface_size();

                let control = world.fetch(control).unwrap();
                let camera_desc =
                    postcard::from_bytes::<CameraDescriptor>(&control.read(world)).unwrap();

                let camera = world.build(CameraDescriptor {
                    size: Size::new(size.width, size.height),
                    ..camera_desc
                });

                let control = control.handle();
                world.insert(Camera::save_write(camera, control));
            }),
        }
    }

    fn save_write(camera: Handle<Camera>, control: Handle<SaveControl>) -> SaveControlWrite {
        SaveControlWrite(Box::new(move |world| {
            let camera = world.fetch(camera).unwrap();
            let control = world.fetch(control).unwrap();

            let bytes = postcard::to_stdvec(&CameraDescriptor {
                size: camera.size,
                center: camera.center,
                zoom: camera.zoom,
            })
            .unwrap();

            control.write(world, &bytes);
        }))
    }
}

pub struct CameraVisits {
    pub views: Vec<ViewId>,
}

impl Element for CameraVisits {}

#[derive(Default)]
pub struct CameraUtils {
    cursor: [f64; 2],

    // camera: PositionFract      = camera.center
    // cursor_in_camera: [f64; 2] = cursor
    anchor: PositionFract,
    cursor_in_anchor: [f64; 2],

    locked: bool,
}

impl CameraUtils {
    /// Adjust zoom value, zooming in/out the anchor.
    pub fn zoom_delta(&mut self, world: &World, delta: Fract) {
        let mut camera = world.single_fetch_mut::<Camera>().unwrap();
        let zoom_center = camera.screen_to_world_absolute(self.cursor);

        let anchor_origin = self.anchor;
        self.anchor = zoom_center;
        self.cursor_in_anchor = [0.0, 0.0];

        camera.zoom += delta;
        drop(camera);

        self.update_locked(world);

        self.anchor = anchor_origin;
        self.update_unlocked(world);
    }

    pub fn cursor(&mut self, world: &World, cursor: [f64; 2]) {
        self.cursor = cursor;
        self.update(world);
    }

    pub fn anchor(&mut self, world: &World, anchor: PositionFract) {
        self.anchor = anchor;
        self.update(world);
    }

    pub fn anchor_on_screen(&mut self, world: &World, anchor_on_screen: [f64; 2]) {
        let camera = world.single_fetch::<Camera>().unwrap();
        let anchor = camera.screen_to_world_absolute(anchor_on_screen);
        drop(camera);
        self.anchor(world, anchor);
    }

    /// Set **locked** to change camera.
    pub fn locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    /// The behavior will depend on previous operations.
    fn update(&mut self, world: &World) {
        if self.locked {
            self.update_locked(world);
        } else {
            self.update_unlocked(world);
        }
    }

    /// resolve `camera.center`
    fn update_locked(&mut self, world: &World) {
        let mut camera = world.single_fetch_mut::<Camera>().unwrap();
        let delta = camera.screen_to_world_relative([
            self.cursor[0] - self.cursor_in_anchor[0],
            self.cursor[1] - self.cursor_in_anchor[1],
        ]);

        camera.center = self.anchor - delta;
    }

    /// resolve `cursor_in_anchor`
    fn update_unlocked(&mut self, world: &World) {
        let camera = world.single_fetch::<Camera>().unwrap();
        let delta = camera.world_to_screen_relative(self.anchor - camera.center);

        self.cursor_in_anchor = [self.cursor[0] - delta[0], self.cursor[1] - delta[1]];
    }
}

impl Element for CameraUtils {}
