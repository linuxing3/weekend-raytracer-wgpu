use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;

use super::{texture::XImageBuffer, WgpuTexture};
use image::{DynamicImage, ImageBuffer};
use imgui::TextureId;

#[derive(Debug)]
pub struct ImguiImage {
    pub texture_id: TextureId,
    pub imgbuf: XImageBuffer,
    pub imgbuf_pin: NonNull<XImageBuffer>,
    pub width: f32,
    pub height: f32,
    _pin: PhantomPinned,
}

impl ImguiImage {
    pub fn new(
        width: f32,
        height: f32,
    ) -> Pin<Box<Self>> {
        // let (width, height) = render_params.viewport_size;
        let texture_id = TextureId::new(0);
        let imgbuf = ImageBuffer::new(width as u32, height as u32);
        let res = ImguiImage {
            texture_id,
            imgbuf,
            imgbuf_pin: NonNull::dangling(),
            width: width as f32,
            height: height as f32,
            _pin: PhantomPinned,
        };
        let mut boxed = Box::pin(res);
        let imgbuf_pin = NonNull::from(&boxed.imgbuf);
        unsafe {
            let mut_ref: Pin<&mut Self> = Pin::as_mut(&mut boxed);
            Pin::get_unchecked_mut(mut_ref).imgbuf_pin = imgbuf_pin;
        }
        boxed
    }

    pub fn imagebuffer_to_bytes(&mut self) -> Vec<u8> {
        unsafe {
            let imgbuf_ptr = self.imgbuf_pin.as_ptr();
            let img = DynamicImage::from((*imgbuf_ptr).clone());
            return img.to_rgba8().to_vec();
        }
    }

    // BUG:
    pub fn allocate_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        unsafe {
            let size = wgpu::Extent3d {
                width: self.width as u32,
                height: self.height as u32,
                depth_or_array_layers: 1,
            };
            let bytes = self.imagebuffer_to_bytes();
            let imgui_texture =
                WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

            self.texture_id = renderer.textures.insert(imgui_texture);
        }
    }

    pub fn update_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        unsafe {
            let size = wgpu::Extent3d {
                width: self.width as u32,
                height: self.height as u32,
                depth_or_array_layers: 1,
            };
            let bytes = self.imagebuffer_to_bytes();
            let imgui_texture =
                WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

            renderer.textures.replace(self.texture_id(), imgui_texture);
        }
    }

    pub fn resize(
        &mut self,
        w: f32,
        h: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        if self.width != w && self.height != h {
            self.width = w;
            self.height = h;
        }
        // NOTE:
        // resize happens instantly, memory must be updated!!!
        self.update_memory(device, queue, renderer);
    }

    pub fn release(&mut self) {
        self.imgbuf_pin = NonNull::dangling();
    }

    pub fn texture_id(&self) -> TextureId {
        self.texture_id
    }
    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn height(&self) -> f32 {
        self.height
    }
}