#![deny(clippy::pedantic, nonstandard_style)]
#![allow(dead_code)]
#![allow(unused_imports)]
use super::{
    math::*, scatter_dielectric, scatter_lambertian, scatter_metal, texture_lookup, GpuCamera,
    GpuMaterial, ImguiImage, Intersection, Ray, RenderParams, Scene, Sphere, TextureDescriptor,
};
use image::{ImageBuffer, Rgb, Rgba};
use nalgebra_glm::{dot, vec3, Vec3};
use num::abs;
use std::pin::Pin;
use std::ptr::null;
use std::{ops::DerefMut, ptr::null_mut};

pub struct ImguiRenderer {
    pub image: Pin<Box<ImguiImage>>,
    pub camera: *mut GpuCamera,
    pub scene: *mut Scene,
    material_data: *const Vec<GpuMaterial>,
    global_texture_data: *const Vec<[f32; 3]>,
}

impl ImguiRenderer {
    pub fn new(
        render_params: &RenderParams,
        camera: *mut GpuCamera,
    ) -> Self {
        let image = ImguiImage::new(400.0, 400.0);
        Self {
            camera,
            image,
            scene: null_mut(),
            material_data: null(),
            global_texture_data: null(),
        }
    }

    pub fn resize(
        &mut self,
        w: f32,
        h: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> bool {
        unsafe {
            let image: Pin<&mut ImguiImage> = Pin::as_mut(&mut self.image);
            let height = image.height();
            let width = image.width();
            if height != h && width != w {
                Pin::get_unchecked_mut(image).resize(width, height, device, queue, renderer);
                return true;
            }
            false
        }
    }

    pub fn render(
        &mut self,
        rp: &RenderParams,
        camera: *mut GpuCamera,
        scene: *mut Scene,
        materials: *const Vec<GpuMaterial>,
        textures: *const Vec<[f32; 3]>,
    ) {
        self.camera = camera;
        self.scene = scene;
        self.material_data = materials;
        self.global_texture_data = textures;
        unsafe {
            let height = (*self.image).height();
            let width = (*self.image).width();
            let imgbuf = (*self.image).imgbuf_pin.as_ptr();
            // A redundant loop to demonstrate reading image data
            for y in 0..height as u32 {
                for x in 0..width as u32 {
                    let pixel = (*imgbuf).get_pixel_mut(x, y);
                    *pixel = self.per_pixel(x, y, rp);
                }
            }
            // set to image
        }
    }

    pub fn per_pixel_lerp(
        &mut self,
        x: u32,
        y: u32,
    ) -> Rgba<u8> {
        let height = (*self.image).height();
        let width = (*self.image).width();
        let u = coord_to_color(x, width as f32);
        let v = coord_to_color(y, height as f32);
        let mut start_color = vec3(u * 255.0, v * 255.0, 25.0);
        let mut final_color = vec3(255.0, 255.0, 255.0);
        let color = glm::lerp(&start_color, &final_color, 0.1);
        vec3_to_rgba8(color)
    }

    pub fn per_pixel(
        &mut self,
        x: u32,
        y: u32,
        render_params: &RenderParams,
    ) -> Rgba<u8> {
        let height = (*self.image).height();
        let width = (*self.image).width();
        // coordinate offset
        let u = coord_to_color(x, width as f32);
        let v = coord_to_color(y, height as f32);
        // random to get noise
        let (uu, vv) = (u + random_f32(), v + random_f32());
        // initialize pixel color with blank color
        let mut pixel_color = Vec3::zeros();

        // create hit info
        let rec = Box::into_raw(Box::new(Intersection::new()));
        unsafe {
            // choose first sphere from scene
            let first_sphere = (*self.scene).spheres[0];
            // make ray from camera
            let mut ray = (*self.camera).make_ray(uu, vv);
            // multisampling
            for i in 0..40 {
                // check hit
                if first_sphere.closest_hit_raw(&ray, 0.001, std::f32::MAX, rec) {
                    // use hit record normal vector as color
                    let mut sampled_color = (*rec).n.normalize() * 255.0 / 2.0;
                    // accumulat color per sample
                    pixel_color += sampled_color;
                    return vec3_to_rgba8(pixel_color);
                }
                // return default background
                return self.per_pixel_lerp(x, y);
            }
            // return default background
            return self.per_pixel_lerp(x, y);
        }
    }

    //
    //  Calculate the color of ray tracing, considering the followings:
    //  1. multitimes bouncing
    //  2. send ray from eye
    //  3. hit the sphere at, got intersection (point vector, normal vector,
    //  etc.)
    //  4. recursively send ray for sampling times with material
    //  color/texture, from p to unit sphere with normal vector lenght as
    //  radius
    //  5. convert normal plus other physical factors
    //  (attenuation, fuzzy refection) to get final color
    //
    //  @params
    //
    //  @ray:   the entre ray
    //  @world: a impl Hittable, which can be hit by ray
    //  @material: materials including metal, dielectric, lambertian, etc
    //  @fuzzy:    fuzzy reflection factor
    //  @depth: limit ray bouncing times
    //
    pub fn ray_color_per_pixel(
        &mut self,
        x: u32,
        y: u32,
        render_params: &RenderParams,
    ) -> Rgba<u8> {
        unsafe {
            let height = (*self.image).height();
            let width = (*self.image).width();

            let u = coord_to_color(x, width as f32);

            let v = coord_to_color(y, height as f32);

            let n_samples = render_params.sampling.num_samples_per_pixel;

            let mut depth: u32 = 10;

            let multipler = 0.5;

            let mut pixel_color = Vec3::zeros();

            // sampling
            for _s in 0..n_samples {
                let (uu, vv) = (u + random_f32(), v + random_f32());

                let mut ray = (*self.camera).make_ray(uu, vv);

                // NOTE: hit info record
                let rec = Box::into_raw(Box::new(Intersection::new()));

                if self.ray_hit_world_raw(&ray, 0.001, f32::MAX, rec) {
                    if depth <= 0 {
                        return vec3_to_rgba8(vec3(0.0, 0.0, 0.0));
                    }

                    depth -= 1;

                    let object_index = (*rec).m;

                    // scatter + attenuation + reflect
                    let scattered_ray = Box::into_raw(Box::new(Ray::new_from_xy(0.0, 0.0)));

                    let mut fuzzy = 0.0;
                    let mut albedo = Vec3::zeros();
                    let refraction_index = 1.5_f32;

                    match object_index {
                        1 | 4 => {
                            if scatter_lambertian(&ray, rec, scattered_ray) {
                                let texture = (*self.material_data)[1].desc1;
                                fuzzy = (*self.material_data)[1].x;
                                albedo = texture_lookup(
                                    texture,
                                    &(*self.global_texture_data),
                                    (*rec).u,
                                    (*rec).v,
                                );
                            }
                        }
                        2 => {
                            if scatter_metal(&ray, rec, scattered_ray) {
                                let texture = (*self.material_data)[2].desc1;
                                fuzzy = (*self.material_data)[2].x;
                                albedo = texture_lookup(
                                    texture,
                                    &(*self.global_texture_data),
                                    (*rec).u,
                                    (*rec).v,
                                );
                            }
                        }
                        3 => {
                            scatter_dielectric(&ray, rec, refraction_index, scattered_ray);
                        }
                        _ => {
                            if scatter_metal(&ray, rec, scattered_ray) {
                                let texture = (*self.material_data)[2].desc1;
                                fuzzy = (*self.material_data)[2].x;
                                albedo = texture_lookup(
                                    texture,
                                    &(*self.global_texture_data),
                                    (*rec).u,
                                    (*rec).v,
                                );
                            }
                        }
                    };

                    // using scattered ray to trace more
                    if self.ray_hit_world_raw(&(*scattered_ray), 0.001, f32::MAX, rec) {
                        let mut sampled_color = (*rec).n.normalize() * 255.0 / 2.0;
                        sampled_color.x *= (*albedo).x * fuzzy;
                        sampled_color.y *= (*albedo).y * fuzzy;
                        sampled_color.z *= (*albedo).z * fuzzy;

                        pixel_color += multipler * sampled_color;

                        return vec3_to_rgba8(pixel_color);
                    }
                }
            }

            vec3_to_rgba8(vec3(v * 255.0, u * 255.0, 255.0))
        }
    }

    pub fn ray_hit_world(
        &mut self,
        ray: &Ray,
        tmin: f32,
        tmax: f32,
        rec: &mut Intersection,
    ) -> bool {
        unsafe {
            let mut temp_rec = Intersection::new();

            let mut hit_anything = false;

            let mut closest_hit = tmax;

            let old_hit = rec.t;

            let world = &(*self.scene).spheres;

            for object in world[..].into_iter() {
                let result = object.closest_hit(&ray, tmin, closest_hit, &mut temp_rec);

                if result.0 {
                    hit_anything = true;

                    closest_hit = old_hit;

                    *rec = *(result.1.unwrap().deref_mut());
                }
            }

            return hit_anything;
        }
    }

    pub fn ray_hit_world_raw(
        &mut self,
        ray: &Ray,
        tmin: f32,
        tmax: f32,
        rec: *mut Intersection,
    ) -> bool {
        unsafe {
            let world = &(*self.scene).spheres;
            let mut temp_rec = Intersection::new();

            let mut hit_anything = false;

            let mut closest_hit = tmax;

            let old_hit = (*rec).t;

            for (index, object) in world[..].into_iter().enumerate() {
                if object.closest_hit_raw(&ray, tmin, closest_hit, &mut temp_rec) {
                    hit_anything = true;
                    closest_hit = old_hit;
                    *rec = temp_rec;
                }
            }

            return hit_anything;
        }
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
