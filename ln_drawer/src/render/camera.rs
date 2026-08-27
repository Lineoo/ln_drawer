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

pub struct CameraUpdated;

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
    pub fn screen_to_world_absolute(&self, point: DVec2) -> I64Vec2 {
        self.center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: DVec2) -> I64Vec2 {
        let scale = self.zoom.q32_as_f64().exp2();
        let pf = delta / scale * self.size.as_dvec2() / 2.0;
        I64Vec2::q32_from_f64(pf)
    }

    #[expect(unused)]
    pub fn world_to_screen_absolute(&self, point: I64Vec2) -> DVec2 {
        self.world_to_screen_relative(point - self.center)
    }

    pub fn world_to_screen_relative(&self, point: I64Vec2) -> DVec2 {
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

pub struct CameraUtils {
    camera_center: I64Vec2,
    camera_zoom: i64,
    camera_cursor: DVec2,
    camera_distance: f64,

    anchor_center: I64Vec2,
    anchor_zoom: i64,
    anchor_cursor: DVec2,
    anchor_distance: f64,

    camera_size: UVec2,
    anchor_lock: bool,
}

impl CameraUtils {
    pub fn new(camera: &Camera) -> CameraUtils {
        CameraUtils {
            camera_center: camera.center,
            camera_zoom: camera.zoom,
            camera_cursor: DVec2::ZERO,
            camera_distance: 1.0,
            anchor_center: I64Vec2::ZERO,
            anchor_zoom: 0,
            anchor_cursor: DVec2::ZERO,
            anchor_distance: 1.0,
            camera_size: camera.size,
            anchor_lock: false,
        }
    }

    pub fn update_from(&mut self, camera: &Camera) {
        self.camera_center = camera.center;
        self.camera_zoom = camera.zoom;
        self.camera_size = camera.size;
    }

    pub fn force_camera_center(&mut self, center: I64Vec2) {
        self.camera_center = center;
    }

    pub fn force_camera_zoom(&mut self, zoom: i64) {
        self.camera_zoom = zoom;
    }

    pub fn camera_cursor_by_camera_center(&mut self, cursor: DVec2) {
        self.camera_cursor = cursor;
        self.camera_center = self.resolve_camera_center();
    }

    pub fn camera_distance_by_camera_zoom_center(&mut self, distance: f64) {
        self.camera_distance = distance;
        self.camera_zoom = self.resolve_camera_zoom();
        self.camera_center = self.resolve_camera_center();
    }

    #[cfg_attr(not(test), expect(unused))]
    pub fn camera_cursor_by_anchor_cursor(&mut self, cursor: DVec2) {
        self.camera_cursor = cursor;
        self.anchor_cursor = self.resolve_anchor_cursor();
    }

    pub fn camera_cursor_by_anchor_center(&mut self, cursor: DVec2) {
        self.camera_cursor = cursor;
        self.anchor_center = self.resolve_anchor_center();
    }

    pub fn camera_distance_by_anchor_zoom_cursor(&mut self, distance: f64) {
        self.camera_distance = distance;
        self.anchor_zoom = self.resolve_anchor_zoom();
        self.anchor_cursor = self.resolve_anchor_cursor();
    }

    #[cfg_attr(not(test), expect(unused))]
    pub fn camera_distance_by_anchor_distance(&mut self, distance: f64) {
        self.camera_distance = distance;
        self.anchor_distance = self.resolve_anchor_distance();
    }

    #[cfg_attr(not(test), expect(unused))]
    pub fn anchor_center(&mut self, center: I64Vec2) {
        self.anchor_center = center;
        if self.anchor_lock {
            self.update_locked();
        }
    }

    #[cfg_attr(not(test), expect(unused))]
    pub fn anchor_zoom(&mut self, zoom: i64) {
        self.anchor_zoom = zoom;
        if self.anchor_lock {
            self.update_locked();
        }
    }

    pub fn anchor_cursor(&mut self, cursor: DVec2) {
        self.anchor_cursor = cursor;
        if self.anchor_lock {
            self.update_locked();
        }
    }

    pub fn anchor_distance(&mut self, distance: f64) {
        self.anchor_distance = distance;
        if self.anchor_lock {
            self.update_locked();
        }
    }

    pub fn camera_size(&mut self, size: UVec2) {
        self.camera_size = size;
    }

    #[expect(unused)]
    pub fn anchor_lock(&mut self, locked: bool) {
        self.anchor_lock = locked;
    }

    pub fn force_clear(&mut self) {
        self.camera_cursor = DVec2::ZERO;
        self.camera_distance = 1.0;
        self.anchor_center = I64Vec2::ZERO;
        self.anchor_zoom = 0;
        self.anchor_cursor = DVec2::ZERO;
        self.anchor_distance = 1.0;
        self.anchor_lock = false;
    }

    pub fn apply_to_camera(&self, world: &World) {
        let mut camera = world.single_fetch_mut::<Camera>().unwrap();
        camera.zoom = self.camera_zoom;
        camera.center = self.camera_center;
        world.queue_trigger(camera.handle(), CameraUpdated);
    }

    /// -> camera_center camera_zoom
    fn update_locked(&mut self) {
        self.camera_zoom = self.resolve_camera_zoom();
        self.camera_center = self.resolve_camera_center();
    }

    fn resolve_camera_center(&mut self) -> I64Vec2 {
        self.anchor_center + self.screen_to_world_relative(self.anchor_cursor - self.camera_cursor)
    }

    fn resolve_camera_zoom(&mut self) -> i64 {
        self.anchor_zoom + i64::q32_from_f64((self.camera_distance / self.anchor_distance).log2())
    }

    fn resolve_anchor_cursor(&mut self) -> DVec2 {
        self.camera_cursor + self.world_to_screen_relative(self.camera_center - self.anchor_center)
    }

    fn resolve_anchor_distance(&mut self) -> f64 {
        self.camera_distance * (self.anchor_zoom - self.camera_zoom).q32_as_f64().exp2()
    }

    fn resolve_anchor_center(&mut self) -> I64Vec2 {
        self.camera_center + self.screen_to_world_relative(self.camera_cursor - self.anchor_cursor)
    }

    fn resolve_anchor_zoom(&mut self) -> i64 {
        self.camera_zoom + i64::q32_from_f64((self.anchor_distance / self.camera_distance).log2())
    }

    fn screen_to_world_relative(&self, delta: DVec2) -> I64Vec2 {
        let scale = self.camera_zoom.q32_as_f64().exp2();
        let pf = delta / scale * self.camera_size.as_dvec2() / 2.0;
        I64Vec2::q32_from_f64(pf)
    }

    fn world_to_screen_relative(&self, delta: I64Vec2) -> DVec2 {
        let scale = self.camera_zoom.q32_as_f64().exp2();
        let pf = delta.q32_as_f64() * 2.0 / self.camera_size.as_dvec2() * scale;
        pf.into()
    }
}

impl Element for MainCamera {}
impl Element for UICamera {}
impl Element for CameraUtils {}

#[cfg(test)]
mod test {
    use super::*;

    const DEFAULT_CAMERA: CameraUtils = CameraUtils {
        camera_center: I64Vec2::ZERO,
        camera_zoom: 0,
        camera_cursor: DVec2::ZERO,
        camera_distance: 1.0,
        anchor_center: I64Vec2::ZERO,
        anchor_zoom: 0,
        anchor_cursor: DVec2::ZERO,
        anchor_distance: 1.0,
        camera_size: UVec2::new(640, 360),
        anchor_lock: false,
    };

    #[test]
    fn test_center() {
        let mut utils = DEFAULT_CAMERA;

        assert_eq!(utils.camera_center.q32_as_f64(), DVec2::new(0., 0.));

        utils.camera_cursor_by_anchor_center(DVec2::new(1.0, 1.0));

        assert_eq!(utils.camera_center.q32_as_f64(), DVec2::new(0., 0.));

        utils.camera_cursor_by_camera_center(DVec2::new(-1.0, -1.0));

        assert_eq!(utils.camera_center.q32_as_f64(), DVec2::new(640., 360.));
    }

    #[test]
    fn test_resolve_anchor_cursor() {
        let mut utils = DEFAULT_CAMERA;

        utils.anchor_center(I64Vec2::q32_from_f64(DVec2::new(0.0, 0.0)));
        utils.camera_cursor_by_anchor_cursor(DVec2::new(1.0, 0.0));
        assert_eq!(utils.anchor_cursor, DVec2::new(1.0, 0.0));

        utils.anchor_center(I64Vec2::q32_from_f64(DVec2::new(0.0, -180.0)));
        utils.camera_cursor_by_anchor_cursor(DVec2::new(1.0, 0.0));
        assert_eq!(utils.anchor_cursor, DVec2::new(1.0, 1.0));
    }

    #[test]
    fn test_resolve_anchor_distance() {
        let mut utils = DEFAULT_CAMERA;

        utils.anchor_zoom(i64::q32_from_f64(0.0));
        utils.camera_distance_by_anchor_distance(1.0);
        assert_eq!(utils.anchor_distance, 1.0);

        utils.anchor_zoom(i64::q32_from_f64(1.0));
        utils.camera_distance_by_anchor_distance(1.0);
        assert_eq!(utils.anchor_distance, 2.0);
    }

    #[test]
    fn test_resolve_anchor_zoom() {
        let mut utils = DEFAULT_CAMERA;

        utils.anchor_distance(1.0);
        utils.camera_distance_by_anchor_zoom_cursor(1.0);
        assert_eq!(utils.anchor_zoom.q32_as_f64(), 0.0);
        assert_eq!(utils.camera_zoom.q32_as_f64(), 0.0);

        utils.anchor_distance(0.5);
        utils.camera_distance_by_anchor_zoom_cursor(1.0);
        assert_eq!(utils.anchor_zoom.q32_as_f64(), -1.0);
        assert_eq!(utils.camera_zoom.q32_as_f64(), 0.0);

        utils.anchor_distance(4.0);
        utils.camera_distance_by_anchor_zoom_cursor(2.0);
        assert_eq!(utils.anchor_zoom.q32_as_f64(), 1.0);
        assert_eq!(utils.camera_zoom.q32_as_f64(), 0.0);
    }
}
