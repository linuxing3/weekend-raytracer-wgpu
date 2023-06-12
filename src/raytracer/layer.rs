use std::f32::consts::{FRAC_1_PI, PI};

use super::{math::*, texture::*, Intersection, Ray, Sphere};
use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::TextureId;
use nalgebra_glm::{acos, atan2, dot, Vec3};

pub struct Layer<'a> {
    texture_id: imgui::TextureId,
    pub vp_size: [f32; 2],
    pub w_title: &'a str,
    pub f_path: &'a str,
    imgbuf: *mut XImageBuffer,
    pub camera: ImguiCamera,
    world: Vec<Sphere>,
}

impl<'a> Layer<'a> {
    pub fn new(
        size: [f32; 2],
        title: &'a str,
        file_path: &'a str,
    ) -> Self {
        let camera = ImguiCamera::default();

        let [width, height] = size;

        let new_buffer: XImageBuffer = ImageBuffer::new(width as u32, height as u32);

        let imgbuf = Box::into_raw(Box::new(new_buffer));

        let sphere1 = Sphere::new(glm::vec3(0.0, 0.0, -1.0), 0.5, 1);
        let sphere2 = Sphere::new(glm::vec3(0.0, -100.5, -1.0), 10.0, 1);

        let world = vec![sphere1, sphere2];

        let texture_id = TextureId::new(0);

        Self {
            texture_id,
            vp_size: size,
            w_title: title,
            f_path: file_path,
            imgbuf,
            camera,
            world,
        }
    }

    pub fn register_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> Option<TextureId> {
        let [width, height] = self.vp_size;

        let imgbuf = self.imgbuf().unwrap();

        let img = DynamicImage::from(imgbuf);

        let bytes: &[u8] = &img.to_rgba8();

        let size = wgpu::Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let imgui_texture: _ =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        self.texture_id = renderer.textures.insert(imgui_texture);

        Some(self.texture_id)
    }

    pub fn texture_id(&mut self) -> &imgui::TextureId {
        &self.texture_id
    }

    pub fn imgbuf(&mut self) -> Option<XImageBuffer> {
        let imgbuf_boxed = unsafe { Box::from_raw(self.imgbuf) };

        Some(*imgbuf_boxed)
    }

    pub fn render(
        &mut self,
        ui: &mut imgui::Ui,
    ) {
        let window = ui.window(self.w_title);

        let mut new_imgui_region_size = None;

        window
            .size(self.vp_size, imgui::Condition::FirstUseEver)
            .build(|| {
                ui.separator();

                ui.text("Imgui Camera parameters");

                ui.slider("origin x", 0.0, 10.0, &mut self.camera.origin.x);

                ui.slider("origin y", 0.0, 10.0, &mut self.camera.origin.y);

                ui.slider("origin.z", -10.0, 10.0, &mut self.camera.origin.z);

                ui.separator();

                new_imgui_region_size = Some(ui.content_region_avail());

                imgui::Image::new(self.texture_id, new_imgui_region_size.unwrap()).build(ui);
            });
    }

    pub fn resize(
        &mut self,
        new_size: [f32; 2],
    ) {
        if self.vp_size != new_size {
            self.vp_size = new_size;
        }

        let [width, height] = self.vp_size;

        let new_imgbuf = ImageBuffer::new(width as u32, height as u32);
        self.imgbuf = Box::into_raw(Box::new(new_imgbuf));

        self.set_data();
    }

    pub fn set_data(&mut self) {
        let [width, height] = self.vp_size;
        unsafe {
            // A redundant loop to demonstrate reading image data
            for y in 0..height as u32 {
                for x in 0..width as u32 {
                    let pixel = (*self.imgbuf).get_pixel_mut(x, y);

                    let u = coord_to_color(x, width);

                    let v = coord_to_color(y, height);

                    *pixel = self.per_pixel(u, v);
                }
            }
        }
    }

    pub fn per_pixel(
        &mut self,
        x: f32,
        y: f32,
    ) -> Rgb<u8> {
        let (u, v) = ((x + 1.0) / 2.0, (y + 1.0) / 2.0);

        // pixel color for multisampling
        for sphere in &self.world {
            let mut _pixel_color = Rgb([0_u8, 0_u8, 0_u8]);
            // multisampling
            for _s in 0..50 {
                // FIXME: include random (-1.0 - 1.0)
                let su = u + 0.010;
                let sv = v + 0.010;
                let mut ray = self.camera.get_ray(su, sv);
                let root = Self::trace_ray(&mut ray, *sphere, 0.0, num::Float::max_value());

                let hit = Self::get_ray_hit(&mut ray, *sphere, root);

                let nn = hit.n.normalize();

                let ray_color = rgb8_from_vec3([nn.x, nn.y, nn.z]);

                _pixel_color = add_rgb8(_pixel_color, ray_color);

                let background_color = rgb8_from_vec3([x, y, 50.0]);
                if hit.t >= 0.0 {
                    return _pixel_color;
                } else {
                    return background_color;
                }
            }
        }

        rgb8_from_vec3([0.0, 0.0, 0.0])
    }

    pub fn set_pixel_with_art_style(
        x: u32,
        y: u32,
        scalex: f32,
        scaley: f32,
    ) -> Rgb<u8> {
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
        width: u32,
        height: u32,
        path: &str,
    ) -> Result<XImageBuffer, TextureError> {
        // Create a new ImgBuf with width: imgx and height: imgy
        let imgbuf = ImageBuffer::new(width, height);

        // Save the image as “fractal.png”, the format is deduced from the path
        imgbuf.save(path).unwrap();

        Ok(imgbuf)
    }

    pub fn ray_intersect_circle(
        ray: &Ray,
        radius: f32,
    ) -> f32 {
        let a = dot(&ray.direction, &ray.direction);

        let b = dot(&ray.origin, &ray.direction);

        let c = dot(&ray.origin, &ray.origin) - radius * radius;

        let discriminant = b * b - a * c;

        if discriminant < 0.0 {
            return -1.0;
        }

        return (-b - num::Float::sqrt(discriminant)) / a;
    }

    pub fn trace_ray(
        ray: &Ray,
        sphere: Sphere,
        tmin: f32,
        tmax: f32,
    ) -> f32 {
        let oc = ray.origin - sphere.center.xyz();

        let a = dot(&ray.direction, &ray.direction);

        let half_b = dot(&oc, &ray.direction);

        let c = dot(&oc, &oc) - sphere.radius * sphere.radius;

        let discriminant = half_b * half_b - a * c;

        if discriminant > 0.0 {
            // closet T
            let mut root = (-half_b - num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                return root;
            }

            // farest T
            root = (-half_b + num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                return root;
            }
        }

        return -1.0;
    }

    pub fn get_ray_hit(
        ray: &Ray,
        sphere: Sphere,
        t: f32,
    ) -> Intersection {
        // p = ray.at(t)
        let p = Self::ray_point_at_t(ray, t);

        // normal = P -c
        let n = (1.0 / sphere.radius) * (p - sphere.center.xyz());

        // ?
        let theta = acos(&-n.yy()).len() as f32;

        // ?
        let phi = atan2(&-n.zz(), &n.xx()).len() as f32 + PI;

        // position.u on viewport
        let u = 0.5 * FRAC_1_PI * phi;

        // position.v on viewport
        let v = FRAC_1_PI * theta;

        return Intersection { p, n, u, v, t };
    }

    pub fn ray_point_at_t(
        ray: &Ray,
        t: f32,
    ) -> Vec3 {
        return ray.origin + t * ray.direction;
    }
}

pub struct ImguiCamera {
    lower_left_color: Vec3,
    origin: Vec3,
    vertical: Vec3,
    horizontal: Vec3,
}

impl ImguiCamera {
    pub fn get_ray(
        &mut self,
        u: f32,
        v: f32,
    ) -> Ray {
        Ray::new(
            self.origin,
            self.lower_left_color + u * self.horizontal + v * self.vertical - self.origin,
        )
    }
}

impl Default for ImguiCamera {
    fn default() -> Self {
        let aspect_ratio = 16.0 / 9.0 as f32;

        let viewport_height = 2.0;

        let viewport_width = aspect_ratio * viewport_height;

        let focal_length = 1.0;

        let origin = glm::vec3(0.0, 0.0, 0.0);

        let horizontal = glm::vec3(viewport_width, 0.0, 0.0);

        let vertical = glm::vec3(0.0, viewport_height, 0.0);

        let lower_left_color =
            origin - horizontal / 2.0 - vertical / 2.0 - glm::vec3(0.0, 0.0, focal_length);

        Self {
            origin,
            vertical,
            horizontal,
            lower_left_color,
        }
    }

    // add code here
}
