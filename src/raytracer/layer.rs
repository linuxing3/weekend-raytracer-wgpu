use std::{
    f32::consts::{FRAC_1_PI, PI},
    ptr::null_mut,
    sync::Arc,
};

use crate::fly_camera::{camera_orientation, FlyCameraController};

use super::{texture::*, Intersection, Ray, Sphere};
use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::TextureId;
use nalgebra_glm::{acos, atan2, dot, Vec3};

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

                let x = (i as f32 / width) * 2.0 - 1.0;

                let y = (j as f32 / height) * 2.0 - 1.0;

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

        let orientation = camera_orientation(&self.camera_controller);

        let origin = self.camera_controller.position;

        let direction = orientation.forward - origin;

        let ray = Ray { origin, direction };

        window
            .size(self.size, imgui::Condition::FirstUseEver)
            .build(|| {

                new_imgui_region_size = Some(ui.content_region_avail());

                ui.text(format!("ray origin x: {}", ray.origin.x));

                ui.text(format!("ray origin y: {}", ray.origin.y));

                ui.text(format!("ray origin z: {}", ray.origin.z));

                ui.text(format!("ray direction x: {}", direction.x));

                ui.text(format!("ray direction y: {}", direction.y));

                ui.text(format!("ray direction z: {}", direction.z));

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

                let x = (i as f32 / width) * 2.0 - 1.0;

                let y = (j as f32 / height) * 2.0 - 1.0;

                *pixel = self.update_pixel(x, y);
            }
        }

        self.imgbuf = Box::into_raw(Box::new(new_imgbuf));
    }

    pub fn update_pixel(&mut self, x : f32, y : f32) -> Rgb<u8> {

        let origin = Vec3::new(0.0, 0.0, 2.0);

        let direction = Vec3::new(x, y, -1.0);

        let ray = Ray { origin, direction };

        let radius = 0.5_f32;

        // closeshit
        let t = Self::ray_intersect_circle(&ray, radius);

        // Normal

        // println!(" Coords: [{}, {}] ", x, y);
        // println!(" Color:  [{}, {}, {} -> {}] ", a, b, c, discriminant);
        let hit_color = Rgb([15.0 as u8, 18.0 as u8, 18.0 as u8]);

        let background_color = Rgb([(x * 255.0) as u8, (y * 255.0) as u8, 1.0 as u8]);

        match t >= 0.0 {
            true => hit_color,
            false => background_color,
        }
    }

    pub fn per_pixel(x : f32, y : f32) -> Rgb<u8> {

        let mut ray_sptr = Arc::new(Ray::new2(x, y));

        let radius = 0.5_f32;

        let sphere = Sphere::new(glm::vec3(0.0, 0.0, -1.0), radius, 1);

        // t
        let t = Self::ray_intersect_sphere(&mut ray_sptr, sphere, 0.0, num::Float::max_value());

        let hit = Self::sphere_intersection(&mut ray_sptr, sphere, t);

        let unit_normal = hit.n.normalize();

        let [r, g, b] : [u8; 3] = [
            ((unit_normal.x) * 255.0) as u8,
            ((unit_normal.y) * 255.0) as u8,
            ((unit_normal.z) * 255.0) as u8,
        ];

        println!(" Color:  [{}, {}, {} ] ", r, g, b);

        // println!(" Coords: [{}, {}] ", x, y);
        let hit_color = Rgb([r, g, b]);

        let background_color = Rgb([(x * 255.0) as u8, (y * 255.0) as u8, 55.0 as u8]);

        match hit.t >= 0.0 {
            true => hit_color,
            false => background_color,
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

    pub fn ray_intersect_circle(ray : &Ray, radius : f32) -> f32 {

        let a = dot(&ray.direction, &ray.direction);

        let b = dot(&ray.origin, &ray.direction);

        let c = dot(&ray.origin, &ray.origin) - radius * radius;

        let discriminant = b * b - a * c;

        if discriminant < 0.0 {

            return -1.0;
        }

        return (-b - num::Float::sqrt(discriminant)) / a;
    }

    pub fn ray_intersect_sphere(ray : &Ray, sphere : Sphere, tmin : f32, tmax : f32) -> f32 {

        let oc = ray.origin - sphere.center.xyz();

        let a = dot(&ray.direction, &ray.direction);

        let b = dot(&oc, &ray.direction);

        let c = dot(&oc, &oc) - sphere.radius * sphere.radius;

        let discriminant = b * b - 4.0 * a * c;

        if discriminant > 0.0 {

            let mut t = (-b - num::Float::sqrt(discriminant)) / a;

            if t < tmax && t > tmin {

                return t;
            }

            t = (-b + num::Float::sqrt(discriminant)) / a;

            if t < tmax && t > tmin {

                return t;
            }
        }

        return -1.0;
    }

    pub fn sphere_intersection(ray : &Ray, sphere : Sphere, t : f32) -> Intersection {

        let p = Self::ray_point_at_parameter(ray, t);

        let n = (1.0 / sphere.radius) * (p - sphere.center.xyz());

        let theta = acos(&-n.yy()).len() as f32;

        let phi = atan2(&-n.zz(), &n.xx()).len() as f32 + PI;

        let u = 0.5 * FRAC_1_PI * phi;

        let v = FRAC_1_PI * theta;

        return Intersection { p, n, u, v, t };
    }

    pub fn ray_point_at_parameter(ray : &Ray, t : f32) -> Vec3 {

        return ray.origin + t * ray.direction;
    }
}
