#![allow(dead_code)]
#![allow(unused_imports)]

use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;

use super::{texture::YImageBuffer, WgpuTexture};
use image::{DynamicImage, ImageBuffer};
use imgui::TextureId;

#[derive(Debug)]
pub struct ImguiImage {
    pub texture_id: TextureId,
    pub imgbuf: YImageBuffer,
    pub imgbuf_pin: NonNull<YImageBuffer>,
    pub width: f32,
    pub height: f32,
    _pin: PhantomPinned,
}

impl ImguiImage {
    pub fn new(
        width: f32,
        height: f32,
    ) -> Pin<Box<Self>> {
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
            // Option 1:
            // let imgbuf_ptr = self.imgbuf_pin.as_ptr();
            // let img = DynamicImage::from((*imgbuf_ptr).clone());
            // Option 2:
            // let img = DynamicImage::from(self.imgbuf.clone());
            // return img.to_rgba8().to_vec();
            // Option 3:
            self.imgbuf.to_vec()
        }
    }

    pub fn generate_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> imgui_wgpu::Texture {
        let size = wgpu::Extent3d {
            width: self.width as u32,
            height: self.height as u32,
            depth_or_array_layers: 1,
        };
        let bytes = self.imgbuf.to_vec();
        let imgui_texture =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        imgui_texture
    }

    // BUG:
    pub fn allocate_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        unsafe {
            let imgui_texture = self.generate_texture(device, queue, renderer);
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
            let imgui_texture = self.generate_texture(device, queue, renderer);
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

mod test {
    use super::*;
    use crate::raytracer::texture::XImageBuffer;
    use crate::raytracer::texture::YImageBuffer;
    use image::Rgb;
    use image::Rgba;

    #[test]
    fn test_imgbuf_to_vec() {
        let width = 2;
        let height = 2;
        let mut imgbuf: XImageBuffer = XImageBuffer::new(width as u32, height as u32);
        for y in 0..height as u32 {
            for x in 0..width as u32 {
                let pixel = imgbuf.get_pixel_mut(x, y);
                *pixel = Rgb([25, 0, 0]);
            }
        }
        let raw_data = vec![25, 0, 0, 25, 0, 0, 25, 0, 0, 25, 0, 0];
        assert_eq!(imgbuf.to_vec()[..], raw_data);
    }

    #[test]
    fn test_imgbuf_to_rgb8() {
        let width = 2;
        let height = 2;
        let mut imgbuf: XImageBuffer = XImageBuffer::new(width as u32, height as u32);
        for y in 0..height as u32 {
            for x in 0..width as u32 {
                let pixel = imgbuf.get_pixel_mut(x, y);
                *pixel = Rgb([25, 0, 0]);
            }
        }
        let image = DynamicImage::from(imgbuf);
        let raw_data = vec![25, 0, 0, 255, 25, 0, 0, 255, 25, 0, 0, 255, 25, 0, 0, 255];
        assert_eq!(image.to_rgba8().to_vec(), raw_data);
    }

    #[test]
    fn test_imgbuf_to_rgba8() {
        let width = 2;
        let height = 2;
        let mut imgbuf: YImageBuffer = YImageBuffer::new(width as u32, height as u32);
        for y in 0..height as u32 {
            for x in 0..width as u32 {
                let pixel = imgbuf.get_pixel_mut(x, y);
                *pixel = Rgba([25, 0, 0, 255]);
            }
        }
        let raw_data = vec![25, 0, 0, 255, 25, 0, 0, 255, 25, 0, 0, 255, 25, 0, 0, 255];
        assert_eq!(imgbuf.to_vec(), raw_data);
    }
}

#[derive(Debug)]
pub struct XImguiImage {
    pub texture_id: TextureId,
    pub imgbuf: *mut YImageBuffer,
    pub width: f32,
    pub height: f32,
}

impl XImguiImage {
    pub fn new(
        width: f32,
        height: f32,
    ) -> Self {
        let texture_id: TextureId = TextureId::new(0);
        let mut buffer = YImageBuffer::new(width as u32, height as u32);
        let imgbuf: *mut YImageBuffer = &mut buffer;
        Self {
            texture_id,
            imgbuf,
            width,
            height,
        }
    }
    pub fn generate_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> imgui_wgpu::Texture {
        let size = wgpu::Extent3d {
            width: self.width as u32,
            height: self.height as u32,
            depth_or_array_layers: 1,
        };
        let bytes = unsafe { (*self.imgbuf).to_vec() };
        let imgui_texture =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        imgui_texture
    }
    pub fn allocate_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        let imgui_texture = self.generate_texture(device, queue, renderer);
        self.texture_id = renderer.textures.insert(imgui_texture);
    }

    pub fn update_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        let imgui_texture = self.generate_texture(device, queue, renderer);
        renderer.textures.replace(self.texture_id, imgui_texture);
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
        self.update_memory(device, queue, renderer);
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
