use std::f32::{
    consts::{FRAC_1_PI, PI},
    MAX,
};

use crate::fly_camera::FlyCameraController;

use super::{
    math::*, texture::*, GpuCamera, Hittable, Intersection, Ray, RenderParams,
    RenderParamsValidationError, Sphere,
};
use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::TextureId;
use nalgebra_glm::{acos, atan2, dot, Vec3};

pub struct Layer {
    texture_id: imgui::TextureId,
    pub vp_size: [f32; 2],
    imgbuf: *mut XImageBuffer,
    pub camera: GpuCamera,
    world: Vec<Sphere>,
}

impl Layer {
    pub fn new(
        size: [f32; 2],
        render_params: &RenderParams,
    ) -> Self {
        // Note: GpuCamera works in Imgui viewport
        let camera = GpuCamera::new(&render_params.camera, (size[0] as u32, size[1] as u32));

        let [width, height] = size;

        let new_buffer: XImageBuffer = ImageBuffer::new(width as u32, height as u32);

        let imgbuf = Box::into_raw(Box::new(new_buffer));

        let sphere1 = Sphere::new(glm::vec3(-3.0, -3.0, -1.0), 2.0, 1);
        // let sphere2 = Sphere::new(glm::vec3(-3.0, -5.0, -1.0), 5.0, 1);
        let world = vec![sphere1];

        let texture_id = TextureId::new(0);

        Self {
            texture_id,
            vp_size: size,
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
        render_params: &RenderParams,
    ) {
        let title = format!("Texture {}", self.texture_id().id());
        let window = ui.window(title);

        let mut new_imgui_region_size = None;
        // Note: GpuCamera works in Imgui viewport
        self.camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);

        window
            .size(self.vp_size, imgui::Condition::FirstUseEver)
            .build(|| {
                new_imgui_region_size = Some(ui.content_region_avail());
                for c in &mut self.camera.eye {
                    if ui.slider("eye", -10.0, 10.0, c) {};
                }
                imgui::Image::new(self.texture_id, new_imgui_region_size.unwrap()).build(ui);
            });
    }

    pub fn resize(
        &mut self,
        render_params: &RenderParams,
    ) {
        let (v_width, v_height) = render_params.viewport_size;
        if self.vp_size[0] != v_width as f32 || self.vp_size[1] != v_height as f32 {
            self.vp_size[0] = v_width as f32;
            self.vp_size[1] = v_height as f32;
            // Note: GpuCamera works in Imgui viewport
            let camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);
            self.camera = camera;

            let new_imgbuf = ImageBuffer::new(v_width, v_height);
            self.imgbuf = Box::into_raw(Box::new(new_imgbuf));

            self.set_data(render_params);
        };
    }

    pub fn set_data(
        &mut self,
        render_params: &RenderParams,
    ) {
        // self.camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);
        let [width, height] = self.vp_size;
        unsafe {
            // A redundant loop to demonstrate reading image data
            for y in 0..height as u32 {
                for x in 0..width as u32 {
                    let pixel = (*self.imgbuf).get_pixel_mut(x, y);

                    let u = coord_to_color(x, width);

                    let v = coord_to_color(y, height);

                    *pixel = self.per_pixel(u, v, render_params);
                }
            }
        }
    }

    // NOTE:
    pub fn per_pixel(
        &mut self,
        x: f32,
        y: f32,
        render_params: &RenderParams,
    ) -> Rgb<u8> {
        let (u, v) = (x, y);

        // hittable world
        for hittable in &self.world {
            let mut pixel_color = Rgb([0_u8, 0_u8, 0_u8]);
            // NOTE:: multisampling
            // https://raytracing.github.io/images/fig-1.07-pixel-samples.jpg
            let sampleing_nums = render_params.sampling.num_samples_per_pixel;
            for _s in 0..sampleing_nums {
                let su = u + random_double();
                let sv = v + random_double();

                // NOTE: make ray from camera eye to sphere
                // https://raytracing.github.io/images/fig-1.04-ray-sphere.jpg
                let mut ray = self.camera.make_ray(su, sv);
                let (_camera_root, camera_hit) =
                    Self::trace_ray(&mut ray, *hittable, 0.0, num::Float::max_value());

                // NOTE: difussion
                // https://raytracing.github.io/images/fig-1.09-rand-vec.jpg
                let target = camera_hit.p + camera_hit.n + random_in_unit_sphere();
                let mut unit_ray = Ray::new(camera_hit.p, target - camera_hit.p);
                // NOTE: make ray from camera-sphere hitting point
                // to some random point in the unit_normal_sphere
                let (_unit_root, unit_hit) =
                    Self::trace_ray(&mut unit_ray, *hittable, 0.0, num::Float::max_value());
                let nn = unit_hit.n.normalize();

                let ray_color = rgb8_from_vec3([nn.x, nn.y, nn.z]);

                pixel_color = plus_rgb8(pixel_color, ray_color);

                if camera_hit.t >= 0.0 {
                    return pixel_color;
                } else {
                    let background_color = rgb8_from_vec3([x, y, 50.0]);
                    return background_color;
                }
            }
        }

        // when world is empty
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

        Rgb([i as u8, i as u8, i as u8])
    }

    pub fn ray_point_at_t(
        ray: &Ray,
        t: f32,
    ) -> Vec3 {
        return ray.origin + t * ray.direction;
    }
}

impl Hittable for Layer {
    // add code here
    fn trace_ray(
        ray: &Ray,
        sphere: Sphere,
        tmin: f32,
        tmax: f32,
    ) -> (f32, Intersection) {
        let oc = ray.origin - sphere.center.xyz();

        let a = dot(&ray.direction, &ray.direction);

        let half_b = dot(&oc, &ray.direction);

        let c = dot(&oc, &oc) - sphere.radius * sphere.radius;

        let discriminant = half_b * half_b - a * c;

        if discriminant > 0.0 {
            // NOTE: closet T
            // https://raytracing.github.io/images/fig-1.04-ray-sphere.jpg
            let mut root = (-half_b - num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                let hit = Self::get_ray_hit(ray, sphere, root);
                return (root, hit);
            }

            // farest T
            root = (-half_b + num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                let hit = Self::get_ray_hit(ray, sphere, root);
                return (root, hit);
            }
        }

        let hit = Self::get_ray_hit(ray, sphere, -1.0);
        return (-1.0, hit);
    }

    fn get_ray_hit(
        ray: &Ray,
        sphere: Sphere,
        t: f32,
    ) -> Intersection {
        // p = ray.at(t)
        let p = Self::ray_point_at_t(ray, t);

        // normal = P -c
        // https://raytracing.github.io/images/fig-1.05-sphere-normal.jpg
        let mut n = (1.0 / sphere.radius) * (p - sphere.center.xyz());

        // front face?
        let f = glm::dot(&ray.direction, &n) < 0.0;
        n = match f {
            true => n,
            false => -n,
        };

        // ?
        let theta = acos(&-n.yy()).len() as f32;

        // ?
        let phi = atan2(&-n.zz(), &n.xx()).len() as f32 + PI;

        // position.u on viewport
        let u = 0.5 * FRAC_1_PI * phi;

        // position.v on viewport
        let v = FRAC_1_PI * theta;

        return Intersection { p, n, u, v, t, f };
    }
}
