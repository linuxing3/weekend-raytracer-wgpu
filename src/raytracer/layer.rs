use std::ptr::null_mut;

use crate::fly_camera::FlyCameraController;

use super::{texture::*, Ray, RenderParams};
use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::TextureId;
use nalgebra_glm::dot;

pub struct Layer<'a> {
    texture_id : imgui::TextureId,
    pub size : [f32; 2],
    pub title : &'a str,
    pub file_path : &'a str,
    imgbuf : *mut XImageBuffer,
    pub camera_controller : FlyCameraController,
}

impl<'a> Layer<'a> {
    pub fn new(size : [f32; 2], title : &'a str, file_path : &'a str) -> Self {

        let camera_controller = FlyCameraController::default();

        let [width, height] = size;

        let mut new_imgbuf = ImageBuffer::new(width as u32, height as u32);

        // A redundant loop to demonstrate reading image data
        for j in 0..height as u32 {

            for i in 0..width as u32 {

                let pixel = new_imgbuf.get_pixel_mut(i, j);

                let x = i as f32 / width;

                let y = j as f32 / height;

                *pixel = Self::per_pixel(x, y);
            }
        }

        let imgbuf = Box::into_raw(Box::new(new_imgbuf));

        let texture_id = TextureId::new(0);

        Self {
            texture_id,
            size,
            title,
            file_path,
            imgbuf,
            camera_controller,
        }
    }

    pub fn update(&mut self, camera : &FlyCameraController) {

        unsafe {

            let mut imgbuf = Box::from_raw(self.imgbuf);

            let [width, height] = self.size;

            // A redundant loop to demonstrate reading image data
            for j in 0..height as u32 {

                for i in 0..width as u32 {

                    let pixel = imgbuf.get_pixel_mut(i, j);

                    let x = i as f32 / width;

                    let y = j as f32 / height;

                    let origin = camera.position;

                    let direction = glm::vec3(x as f32, x as f32, -1.0);

                    let ray = Ray { origin, direction };

                    let radius = 0.5_f32;

                    let a = dot(&ray.direction, &ray.direction);

                    let b = 2.0 * dot(&ray.origin, &ray.direction);

                    let c = dot(&ray.origin, &ray.origin) - radius * radius;

                    let discriminant = b * b - 4.0 * a * c;

                    *pixel = match discriminant >= 0.0 {
                        true => Rgb([125.0 as u8, 18.0 as u8, 18.0 as u8]),
                        false => Rgb([(x * 255.0) as u8, (y * 255.0) as u8, 55.0 as u8]),
                    };
                }
            }
        }
    }

    pub fn register_texture(
        &mut self,
        device : &wgpu::Device,
        queue : &wgpu::Queue,
        renderer : &mut imgui_wgpu::Renderer,
    ) -> Option<TextureId> {

        let imgbuf_boxed = unsafe {

            Box::from_raw(self.imgbuf)
        };

        let (width, height) = imgbuf_boxed.dimensions();

        let img = DynamicImage::from(*imgbuf_boxed);

        let bytes : &[u8] = &img.to_rgba8();

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers : 1,
        };

        let imgui_texture : _ =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        self.texture_id = renderer.textures.insert(imgui_texture);

        Some(self.texture_id)
    }

    pub fn texture_id(&mut self) -> &imgui::TextureId { &self.texture_id }

    pub fn img_buf(&mut self) -> XImageBuffer {

        let imgbuf_boxed = unsafe {

            Box::from_raw(self.imgbuf)
        };

        let mut imgbuf = *imgbuf_boxed;

        imgbuf
    }

    pub fn render(&mut self, ui : &mut imgui::Ui) {

        let window = ui.window(self.title);

        let mut new_imgui_region_size = None;

        window
            .size(self.size, imgui::Condition::FirstUseEver)
            .build(|| {

                new_imgui_region_size = Some(ui.content_region_avail());

                imgui::Image::new(self.texture_id, new_imgui_region_size.unwrap()).build(ui);
            });
    }

    pub fn resize(&mut self, new_size : [f32; 2]) {

        if self.size != new_size {

            self.size = new_size;
        }

        self.imgbuf = null_mut();

        let [width, height] = new_size;

        let mut new_imgbuf = ImageBuffer::new(width as u32, height as u32);

        // A redundant loop to demonstrate reading image data
        for j in 0..height as u32 {

            for i in 0..width as u32 {

                let pixel = new_imgbuf.get_pixel_mut(i, j);

                let x = i as f32 / width;

                let y = j as f32 / height;

                *pixel = self.update_pixel(x, y);
            }
        }

        self.imgbuf = Box::into_raw(Box::new(new_imgbuf));
    }

    pub fn update_pixel(&mut self, x : f32, y : f32) -> Rgb<u8> {

        let ray = Ray {
            origin : self.camera_controller.position,
            direction : glm::vec3(x, y, -1.0),
        };

        let radius = 0.5_f32;

        let a = dot(&ray.direction, &ray.direction);

        let b = 2.0 * dot(&ray.origin, &ray.direction);

        let c = dot(&ray.origin, &ray.origin) - radius * radius;

        let discriminant = b * b - 4.0 * a * c;

        // println!(" Coords: [{}, {}] ", x, y);
        // println!(" Color:  [{}, {}, {} -> {}] ", a, b, c, discriminant);

        match discriminant >= 0.0 {
            true => Rgb([125.0 as u8, 18.0 as u8, 18.0 as u8]),
            false => Rgb([(x * 255.0) as u8, (y * 255.0) as u8, 55.0 as u8]),
        }
    }

    pub fn per_pixel(x : f32, y : f32) -> Rgb<u8> {

        let origin = glm::vec3(1.5, 1.5, -4.0);

        let direction = glm::vec3(x, y, -1.0);

        let ray = Ray { origin, direction };

        let radius = 0.5_f32;

        let a = dot(&ray.direction, &ray.direction);

        let b = 2.0 * dot(&ray.origin, &ray.direction);

        let c = dot(&ray.origin, &ray.origin) - radius * radius;

        let discriminant = b * b - 4.0 * a * c;

        // println!(" Coords: [{}, {}] ", x, y);
        // println!(" Color:  [{}, {}, {} -> {}] ", a, b, c, discriminant);

        match discriminant >= 0.0 {
            true => Rgb([125.0 as u8, 18.0 as u8, 18.0 as u8]),
            false => Rgb([(x * 255.0) as u8, (y * 255.0) as u8, 55.0 as u8]),
        }
    }

    pub fn set_pixel_with_art_style(x : u32, y : u32, scalex : f32, scaley : f32) -> Rgb<u8> {

        let cx = y as f32 * scalex - 1.5;

        let cy = x as f32 * scaley - 1.5;

        let c = num::complex::Complex::new(-0.4, 0.6);

        let mut z = num::complex::Complex::new(cx, cy);

        let mut i = 0;

        while i < 255 && z.norm() <= 2.0 {

            z = z * z + c;

            i += 1;
        }

        image::Rgb([i as u8, i as u8, i as u8])
    }

    pub fn new_from_image_buffer(
        width : u32,
        height : u32,
        path : &str,
    ) -> Result<XImageBuffer, TextureError> {

        // Create a new ImgBuf with width: imgx and height: imgy
        let imgbuf = ImageBuffer::new(width, height);

        // Save the image as “fractal.png”, the format is deduced from the path
        imgbuf.save(path).unwrap();

        Ok(imgbuf)
    }
}
