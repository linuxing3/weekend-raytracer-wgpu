use super::{
    math::*, GpuCamera, ImguiImage, Intersection, Ray, RenderParams, Scene, Sphere,
    TextureDescriptor,
};
use image::Rgb;
use nalgebra_glm::{dot, vec3, Vec3};
use std::pin::Pin;
use std::{ops::DerefMut, ptr::null_mut};

pub struct ImguiRenderer {
    pub image: Pin<Box<ImguiImage>>,
    pub camera: *mut GpuCamera,
    pub scene: *mut Scene,
}

impl ImguiRenderer {
    pub fn new(
        render_params: &RenderParams,
        camera: *mut GpuCamera,
    ) -> Self {
        let image = ImguiImage::new(10.0, 10.0);
        Self {
            camera,
            image,
            scene: null_mut(),
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
        unsafe {
            let image: Pin<&mut ImguiImage> = Pin::as_mut(&mut self.image);
            let height = image.height();
            let width = image.width();
            if height != h && width != w {
                Pin::get_unchecked_mut(image).resize(width, height, device, queue, renderer);
            }
        }
    }

    pub fn render(
        &mut self,
        rp: &RenderParams,
        camera: *mut GpuCamera,
        scene: *mut Scene,
    ) {
        self.camera = camera;
        self.scene = scene;
        unsafe {
            let height = (*self.image).height();
            let width = (*self.image).width();
            let imgbuf = (*self.image).imgbuf_pin.as_ptr();
            // A redundant loop to demonstrate reading image data
            for y in 0..height as u32 {
                for x in 0..width as u32 {
                    let pixel = (*imgbuf).get_pixel_mut(x, y);
                    *pixel = self.ray_color_per_pixel(x, y, rp);
                    println!("r{} g{} b{}", pixel.0[0], pixel.0[1], pixel.0[2]);
                }
            }
            // set to image
        }
    }
    /**
     *
     * Calculate the color of ray tracing, considering the followings:
     * 1. multitimes bouncing
     * 2. send ray from eye
     * 3. hit the sphere at, got intersection (point vector, normal vector,
     * etc.) 4. recursively send ray for sampling times with material
     * color/texture, from p to unit sphere with normal vector lenght as
     * radius 5. convert normal plus other physical factors
     * (attenuation, fuzzy refection) to get final color
     *
     * @params
     *
     * @ray:   the entre ray
     * @world: a impl Hittable, which can be hit by ray
     * @material: materials including metal, dielectric, lambertian, etc
     * @fuzzy:    fuzzy reflection factor
     * @depth: limit ray bouncing times
     */

    pub fn ray_color_per_pixel(
        &mut self,
        x: u32,
        y: u32,
        render_params: &RenderParams,
    ) -> Rgb<u8> {
        unsafe {
            let (width, height) = render_params.viewport_size;

            let u = coord_to_color(x, width as f32);

            let v = coord_to_color(y, height as f32);

            let n_samples = render_params.sampling.num_samples_per_pixel;

            let mut depth: u32 = 20;

            let multipler = 0.5;

            let mut pixel_color = vec3(0.0, 0.0, 0.0);

            // sampleing
            for _s in 0..n_samples {
                let (uu, vv) = (u + random_f32(), v + random_f32());

                let mut ray = (*self.camera).make_ray(uu, vv);

                // NOTE: hit info record
                let rec = Box::into_raw(Box::new(Intersection::new()));
                let world = &(*self.scene).spheres;

                // if self.ray_hit_world(&ray, 0.001, f32::MAX, &mut rec) {
                if self.ray_hit_world_raw(&ray, world.clone(), 0.001, f32::MAX, rec) {
                    if depth <= 0 {
                        return vec3_to_rgb8(vec3(0.0, 0.0, 0.0));
                    }

                    depth -= 1;

                    // scatter + attenuation + reflect
                    let scattered_ray = Box::into_raw(Box::new(Ray::new_from_xy(0.0, 0.0)));

                    let mut texture = TextureDescriptor::empty();
                    let mut fuzzy = 0_f32;
                    let mut albedo = Vec3::zeros();

                    // if scatter_metal(&ray, rec, scattered_ray) {
                    //     texture = self.material_data[2].desc1;
                    //     fuzzy = self.material_data[2].x;
                    //     albedo =
                    //         texture_lookup(texture, &self.global_texture_data, (*rec).u, (*rec).v);
                    // }

                    let light_dir = vec3(5.0, -3.0, 2.0).normalize();
                    let light_dir_rev = (*rec).p - light_dir;
                    let mut light_theta = dot(&(*rec).n, &light_dir_rev);
                    if light_theta < 0.0 {
                        light_theta = 0.0
                    };
                    // let light_intensity = max(light_theta, 0_f32);

                    // if scatter_lambertian(&ray, rec, scattered_ray) {
                    //     texture = self.material_data[1].desc1;
                    //     fuzzy = self.material_data[1].x;
                    //     albedo =
                    //         texture_lookup(texture, &self.global_texture_data, (*rec).u, (*rec).v);
                    // }

                    if self.ray_hit_world_raw(
                        &(*scattered_ray),
                        world.clone(),
                        0.001,
                        f32::MAX,
                        rec,
                    ) {
                        let mut sampled_color = (*rec).n.normalize() * 255.0 / 2.0;
                        sampled_color.x *= (*albedo).x * fuzzy;
                        sampled_color.y *= (*albedo).y * fuzzy;
                        sampled_color.z *= (*albedo).z * fuzzy;

                        pixel_color += multipler * sampled_color;

                        return vec3_to_rgb8(pixel_color);
                    }
                }
            }

            vec3_to_rgb8(vec3(v * 255.0, u * 255.0, 255.0))
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
        world: Vec<Sphere>,
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
