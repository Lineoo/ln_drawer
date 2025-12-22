#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexUniform {
    pub origin: [i32; 2],
    pub extend: [u32; 2],
}
