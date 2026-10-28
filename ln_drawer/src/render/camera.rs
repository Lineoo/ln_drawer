use glam::{DVec2, I64Vec2, IVec2, Mat2, UVec2, Vec2};
use ln_world::{Descriptor, Element, Handle, World};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    *,
};
use winit::event::WindowEvent;

use crate::{
    lnwin::Lnwindow,
    measures::{FI64Ext, IRect},
    render::{Render, RenderDrawControl},
    tools::collider::ToolControl,
};

pub struct Camera {
    pub center: I64Vec2,
    pub zoom: i64,

    // responsible by lnwindow/container
    pub surface_size: UVec2,
    pub global_center: I64Vec2,
    pub global_zoom: i64,

    pub bind: BindGroup,
    pub uniform: Buffer,

    pub renderers: Vec<Handle<RenderDrawControl>>,
    pub colliders: Vec<Handle<ToolControl>>,

    queue: Queue,
}

#[derive(Debug)]
pub struct CameraDescriptor {
    pub center: I64Vec2,
    pub zoom: i64,
}

pub struct CameraBind {
    pub layout: BindGroupLayout,
}

pub struct SetCameraOutdated;
pub struct CameraOutdated;

pub struct PageCamera(pub Handle<Camera>);
pub struct PageCameraUtils(pub Handle<CameraUtils>);

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PageCameraArchive {
    pub center: I64Vec2,
    pub zoom: i64,
}

pub struct UICamera(pub Handle<Camera>);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    center: IVec2,
    center_fract: UVec2,
    transform: Mat2,
    inverse: Mat2,
    size: Vec2,
    _pad: u64,
}

impl Descriptor for CameraDescriptor {
    type Target = Camera;

    fn when_build(self, world: &World) -> Self::Target {
        let render = world.single_fetch::<Render>();
        let binding = world.single_fetch::<CameraBind>();

        let device = &render.device;
        let size = render.target_size();

        let transform = Mat2::from_scale_angle(
            Vec2::splat(self.zoom.q32_as_f64().exp2() as f32) * 2. / size.as_vec2(),
            0.0,
        );
        let translate = self.center;
        let uniform = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("camera_uniform"),
            contents: bytemuck::bytes_of(&CameraUniform {
                center: translate.q32_floor(),
                center_fract: translate.q32_fract(),
                transform,
                inverse: transform.inverse(),
                size: size.as_vec2(),
                _pad: 0,
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

        Camera {
            center: self.center,
            zoom: self.zoom,
            // TODO this is not global, obviously
            global_center: self.center,
            global_zoom: self.zoom,
            surface_size: size,
            bind,
            uniform,
            renderers: Vec::new(),
            colliders: Vec::new(),
            queue: render.queue.clone(),
        }
    }
}

impl Element for Camera {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        let lnwindow = world.single::<Lnwindow>();
        let obs = world.observer(lnwindow, move |event: &WindowEvent, world| {
            if let WindowEvent::SurfaceResized(_) = event {
                world.queue_trigger(this, SetCameraOutdated);
            }
        });
        world.dependency(obs, this);
    }
}

impl Element for CameraBind {}

impl Camera {
    #[inline]
    pub fn screen_to_world(&self, point: DVec2) -> I64Vec2 {
        self.global_center + self.screen_to_world_relative(point)
    }

    pub fn screen_to_world_relative(&self, delta: DVec2) -> I64Vec2 {
        let scale = self.global_zoom.q32_as_f64().exp2();
        let pf = delta / scale * self.surface_size.as_dvec2() / 2.0;
        I64Vec2::q32_from_f64(pf)
    }

    pub fn world_to_screen(&self, point: I64Vec2) -> DVec2 {
        self.world_to_screen_relative(point - self.global_center)
    }

    pub fn world_to_screen_relative(&self, point: I64Vec2) -> DVec2 {
        let scale = self.global_zoom.q32_as_f64().exp2();
        let pf = point.q32_as_f64() * 2.0 / self.surface_size.as_dvec2() * scale;
        pf.into()
    }

    // TODO #[deprecated = "view_rect should depend on scope size not surface size"]
    pub fn world_view_rect(&self) -> IRect {
        Self::view_rect(self.zoom, self.surface_size, self.center)
    }

    // TODO #[deprecated = "view_rect should depend on scope size not surface size"]
    pub fn view_rect(zoom: i64, size: UVec2, center: I64Vec2) -> IRect {
        let scale = zoom.q32_as_f64().exp2();
        let view_size = size.as_dvec2() / scale * 0.5;
        IRect::new_half(center.q32_round(), view_size.ceil().as_uvec2())
    }

    pub fn init(world: &World) {
        let render = world.single_fetch::<Render>();
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

    pub fn upload(&self) {
        let transform = Mat2::from_scale_angle(
            Vec2::splat(self.global_zoom.q32_as_f64().exp2() as f32) * 2.
                / self.surface_size.as_vec2(),
            0.0,
        );
        let translate = self.global_center;
        self.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&CameraUniform {
                center: translate.q32_floor().into(),
                center_fract: translate.q32_fract().into(),
                transform,
                inverse: transform.inverse(),
                size: self.surface_size.as_vec2(),
                _pad: 0,
            }),
        );
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
            camera_size: camera.surface_size,
            anchor_lock: false,
        }
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
        self.camera_distance = distance.max(1e-5);
        self.camera_zoom = self.resolve_camera_zoom();
        self.camera_center = self.resolve_camera_center();
    }

    pub fn camera_cursor_by_anchor_cursor(&mut self, cursor: DVec2) {
        self.camera_cursor = cursor;
        self.anchor_cursor = self.resolve_anchor_cursor();
    }

    pub fn camera_cursor_by_anchor_center(&mut self, cursor: DVec2) {
        self.camera_cursor = cursor;
        self.anchor_center = self.resolve_anchor_center();
    }

    pub fn camera_distance_by_anchor_zoom_cursor(&mut self, distance: f64) {
        self.camera_distance = distance.max(1e-5);
        self.anchor_zoom = self.resolve_anchor_zoom();
        self.anchor_cursor = self.resolve_anchor_cursor();
    }

    pub fn camera_distance_by_anchor_distance(&mut self, distance: f64) {
        self.camera_distance = distance.max(1e-5);
        self.anchor_distance = self.resolve_anchor_distance();
    }

    pub fn anchor_center(&mut self, center: I64Vec2) {
        self.anchor_center = center;
        if self.anchor_lock {
            self.update_locked();
        }
    }

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
        self.anchor_distance = distance.max(1e-5);
        if self.anchor_lock {
            self.update_locked();
        }
    }

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

    pub fn read_camera(&mut self, world: &World, camera: Handle<Camera>) {
        let camera = world.fetch(camera);
        self.camera_zoom = camera.zoom;
        self.camera_center = camera.center;
        self.camera_size = camera.surface_size;
    }

    pub fn write_camera(&self, world: &World, camera: Handle<Camera>) {
        world.queue_trigger(camera, SetCameraOutdated);
        let mut camera = world.fetch_mut(camera);
        camera.zoom = self.camera_zoom;
        camera.center = self.camera_center;
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

impl Element for PageCamera {}
impl Element for UICamera {}
impl Element for PageCameraUtils {}
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
