use std::{env, path::PathBuf};

use wgpu::*;
use winit::dpi::PhysicalSize;

pub struct Export {
    path: PathBuf,
    buffer: Buffer,
    bytes_per_row: u32,
    width: u32,
    height: u32,
    frame: u64,
    target_frame: u64,
    exported: bool,
}

impl Export {
    pub fn new(device: &Device, size: PhysicalSize<u32>, path: PathBuf) -> Self {
        let target_frame = env::var("LNDRAWER_EXPORT_FRAME")
            .ok()
            .and_then(|frame| frame.parse().ok())
            .unwrap_or(1);

        let width = size.width.max(1);
        let height = size.height.max(1);
        let (buffer, bytes_per_row) = Export::create_buffer(device, width, height);

        Export {
            path,
            buffer,
            bytes_per_row,
            width,
            height,
            frame: 0,
            target_frame,
            exported: false,
        }
    }

    fn create_buffer(device: &Device, width: u32, height: u32) -> (Buffer, u32) {
        let bytes_per_row =
            (width * 4).div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("export_readback"),
            size: (bytes_per_row as u64) * (height as u64),
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        (buffer, bytes_per_row)
    }

    pub fn resize(&mut self, device: &Device, size: PhysicalSize<u32>) {
        let width = size.width.max(1);
        let height = size.height.max(1);
        let (buffer, bytes_per_row) = Export::create_buffer(device, width, height);

        self.buffer = buffer;
        self.bytes_per_row = bytes_per_row;
        self.width = width;
        self.height = height;
    }

    pub fn step(&mut self) -> bool {
        self.frame += 1;
        !self.exported && self.frame >= self.target_frame
    }

    pub fn encode_copy(&self, encoder: &mut CommandEncoder, texture: &Texture) {
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn save(&mut self, device: &Device) {
        let slice = self.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| tx.send(result).unwrap());
        device.poll(PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();

        let data = slice.get_mapped_range().unwrap();
        let row_bytes = self.width as usize * 4;
        let mut pixels = Vec::with_capacity(row_bytes * self.height as usize);
        for line in data.chunks(self.bytes_per_row as usize) {
            pixels.extend_from_slice(&line[..row_bytes]);
        }
        drop(data);
        self.buffer.unmap();

        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).unwrap();
        }

        let file = std::fs::File::create(&self.path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();

        self.exported = true;
        log::info!("exported frame to {}", self.path.display());
    }
}
