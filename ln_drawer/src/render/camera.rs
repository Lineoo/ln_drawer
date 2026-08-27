use glam::{DVec2, I64Vec2, UVec2};
use ln_world::{Descriptor, Element, Handle, World};
use redb::{ReadableDatabase, ReadableTable, TableDefinition};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};
use winit::event::WindowEvent;

use crate::{
    lnwin::Lnwindow,
    measures::{FI64Ext, Rectangle},
    render::Render,
    save::{Autosave, SaveDatabase},
};

const TABLE_CAMERA: TableDefinition<&str, &[u8]> = TableDefinition::new("camera");

pub struct Camera {
    pub size: UVec2,
    pub center: I64Vec2,
    pub zoom: i64,

    pub bind: BindGroup,
    pub uniform: Buffer,

    queue: Queue,
}

pub struct CameraBind {
    pub layout: BindGroupLayout,
}

pub struct MainCamera(pub Handle<Camera>);

pub struct UICamera(pub Handle<Camera>);

#[expect(unused)]
pub struct CameraPositionChanged {
    pub from: I64Vec2,
    pub here: I64Vec2,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct CameraDescriptor {
    pub size: UVec2,
    pub center: I64Vec2,
    pub zoom: i64,
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
        let binding = world.single_fetch::<CameraBind>().unwrap();
        let device = &render.device;

        let uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("camera_uniform"),
            contents: bytemuck::bytes_of(&CameraUniform {
                size: self.size.into(),
                center: self.center.q32_floor().into(),
                center_fract: self.center.q32_fract().into(),
                zoom: self.zoom.q32_floor(),
                zoom_fract: self.zoom.q32_fract(),
            }),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let bind = device.create_bind_group(&BindGroupDescriptor {
            label: Some("camera_bind"),
            layout: &binding.layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &uniform,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        world.insert(Camera {
            size: self.size,
            center: self.center,
            zoom: self.zoom,
            uniform,
            bind,
            queue: render.queue.clone(),
        })
    }
}

impl Element for Camera {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>().unwrap();
        world.observer(lnwindow, move |event: &WindowEvent, world| {
            if let WindowEvent::SurfaceResized(size) = event {
                let mut camera = world.fetch_mut(this).unwrap();

                camera.size.x = size.width;
                camera.size.y = size.height;
            }
        });
    }

    fn when_modify(&mut self, world: &World, _this: Handle<Self>) {
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&CameraUniform {
                size: UVec2::new(self.size.x.max(1), self.size.y.max(1)).into(),
                center: self.center.q32_floor().into(),
                center_fract: self.center.q32_fract().into(),
                zoom: self.zoom.q32_floor(),
                zoom_fract: self.zoom.q32_fract(),
            }),
        );

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        lnwindow.window.request_redraw();
    }
}

impl Element for CameraBind {}

impl Camera {
    #[inline]
    pub fn screen_to_world_absolute(&self, point: [f64; 2]) -> I64Vec2 {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: [f64; 2]) -> I64Vec2 {
        let scale = self.zoom.q32_as_f64().exp2();
        let pf = DVec2::from(delta) / scale * self.size.as_dvec2() / 2.0;
        I64Vec2::q32_from_f64(pf)
    }

    #[expect(unused)]
    pub fn world_to_screen_absolute(&self, point: I64Vec2) -> [f64; 2] {
        self.world_to_screen_relative(point - self.center)
    }

    pub fn world_to_screen_relative(&self, point: I64Vec2) -> [f64; 2] {
        let scale = self.zoom.q32_as_f64().exp2();
        let pf = point.q32_as_f64() * 2.0 / self.size.as_dvec2() * scale;
        pf.into()
    }

    pub fn world_view_rect(&self) -> Rectangle {
        Self::manual_view_rect(self.zoom, self.size, self.center)
    }

    pub fn manual_view_rect(zoom: i64, size: UVec2, center: I64Vec2) -> Rectangle {
        let scale = zoom.q32_as_f64().exp2();
        let view_size = size.as_dvec2() / scale * 0.5;
        Rectangle::new_half(center.q32_round(), view_size.ceil().as_uvec2())
    }

    pub fn init(world: &mut World) {
        let render = world.single_fetch::<Render>().unwrap();
        let device = &render.device;

        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("camera_bind"),
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

        world.insert(CameraBind { layout });
    }

    pub fn build_from_save(world: &World, name: &str) -> Handle<Camera> {
        Camera::try_build_from_save(world, name).unwrap()
    }

    fn try_build_from_save(world: &World, name: &str) -> Result<Handle<Camera>, redb::Error> {
        let db = world.single_fetch::<SaveDatabase>().unwrap();
        Camera::build_default_if_empty(&db, name)?;

        let read = db.0.begin_read()?;
        let table = read.open_table(TABLE_CAMERA)?;
        let bytes = table.get(name)?.unwrap();
        let camera_desc = postcard::from_bytes::<CameraDescriptor>(bytes.value()).unwrap();

        let lnwindow = world.single_fetch::<Lnwindow>().unwrap();
        let size = lnwindow.window.surface_size();
        let camera = world.build(CameraDescriptor {
            size: UVec2::new(size.width, size.height),
            ..camera_desc
        });

        world.insert(Camera::autosave(camera, name));

        Ok(camera)
    }

    fn build_default_if_empty(db: &SaveDatabase, name: &str) -> Result<(), redb::Error> {
        let write = db.0.begin_write()?;
        {
            let mut table = write.open_table(TABLE_CAMERA)?;
            if table.get(name)?.is_some() {
                return Ok(());
            }

            let bytes = postcard::to_stdvec(&CameraDescriptor::default()).unwrap();

            table.insert(name, &bytes[..])?;
        }

        write.commit()?;
        Ok(())
    }

    fn autosave(camera: Handle<Camera>, name: &str) -> Autosave {
        let name_owned = String::from(name);
        Autosave(Box::new(move |world, write| {
            let camera = world.fetch(camera).unwrap();

            let bytes = postcard::to_stdvec(&CameraDescriptor {
                size: camera.size,
                center: camera.center,
                zoom: camera.zoom,
            })
            .unwrap();

            let mut table = write.open_table(TABLE_CAMERA).unwrap();
            table.insert(&name_owned[..], &bytes[..]).unwrap();
        }))
    }
}

#[derive(Default)]
pub struct CameraUtils {
    cursor: [f64; 2],

    // camera: PositionFract      = camera.center
    // cursor_in_camera: [f64; 2] = cursor
    anchor: I64Vec2,
    cursor_in_anchor: [f64; 2],

    locked: bool,
}

impl CameraUtils {
    /// Adjust zoom value, zooming in/out the anchor.
    pub fn zoom_delta(&mut self, world: &World, delta: i64) {
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

    pub fn anchor(&mut self, world: &World, anchor: I64Vec2) {
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

        let dest = self.anchor - delta;
        world.queue_trigger(
            camera.handle(),
            CameraPositionChanged {
                from: camera.center,
                here: dest,
            },
        );
        camera.center = dest;
    }

    /// resolve `cursor_in_anchor`
    fn update_unlocked(&mut self, world: &World) {
        let camera = world.single_fetch::<Camera>().unwrap();
        let delta = camera.world_to_screen_relative(self.anchor - camera.center);

        self.cursor_in_anchor = [self.cursor[0] - delta[0], self.cursor[1] - delta[1]];
    }
}

impl Element for MainCamera {}
impl Element for UICamera {}
impl Element for CameraUtils {}
