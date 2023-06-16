use std::ops::DerefMut;

use super::{
    math::*, texture::*, GpuCamera, Intersection, Material, Metal, Ray, RenderParams, Scatter,
    Scatterable, Scene, Sphere,
};

use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::TextureId;
use nalgebra_glm::{vec3, Vec3};

pub struct Color {
    data: Vec3,
}

impl Color {
    pub fn mul(
        &mut self,
        v: Vec3,
    ) {
        self.data.x *= v.x;
        self.data.y *= v.y;
        self.data.z *= v.z;
    }
}

impl Color {
    pub fn new() -> Self {
        Self {
            data: vec3(0.0, 0.0, 0.0),
        }
    }
}

pub struct Layer {
    pub texture_id: imgui::TextureId,
    pub vp_size: [f32; 2],
    imgbuf: *mut XImageBuffer,
    pub camera: GpuCamera,
    pub world: Vec<Box<Sphere>>,
    pub materials: Vec<Material>,
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

        // Generating hittable objects
        let scene = Self::scene();
        let world = scene.spheres[..]
            .into_iter()
            .map(|s| Box::new(s.clone()))
            .collect();
        let materials = scene.materials;

        let texture_id = TextureId::new(0);

        Self {
            texture_id,
            vp_size: size,
            imgbuf,
            camera,
            world,
            materials,
        }
    }

    pub fn scene() -> Scene {
        let materials = vec![
            Material::Checkerboard {
                even: Texture::new_from_color(glm::vec3(0.5_f32, 0.7_f32, 0.8_f32)),
                odd: Texture::new_from_color(glm::vec3(0.9_f32, 0.9_f32, 0.9_f32)),
            },
            Material::Lambertian {
                albedo: Texture::new_from_image("assets/moon.jpeg")
                    .expect("Hardcoded path should be valid"),
            },
            Material::Metal {
                albedo: Texture::new_from_color(glm::vec3(1_f32, 0.85_f32, 0.57_f32)),
                fuzz: 0.4_f32,
            },
            Material::Dielectric {
                refraction_index: 1.5_f32,
            },
            Material::Lambertian {
                albedo: Texture::new_from_image("assets/earthmap.jpeg")
                    .expect("Hardcoded path should be valid"),
            },
        ];

        let spheres = vec![
            Sphere::new(glm::vec3(5.0, 1.2, -1.5), 1.2, 4_u32),
            Sphere::new(glm::vec3(0.0, -500.0, -1.0), 500.0, 0_u32),
            Sphere::new(glm::vec3(0.0, 1.0, 0.0), 1.0, 3_u32),
            Sphere::new(glm::vec3(-5.0, 1.0, 0.0), 1.0, 2_u32),
            Sphere::new(glm::vec3(2.0, -1.0, 0.0), 2.0, 3_u32),
            Sphere::new(glm::vec3(5.0, 0.8, 1.5), 0.8, 1_u32),
        ];

        Scene { spheres, materials }
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

        let imgui_texture =
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

    pub fn update_camera(
        &mut self,
        render_params: &RenderParams,
    ) {
        self.camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);
    }

    pub fn render_draw_list(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        self.update_camera(render_params);

        let title = format!("Texture {}", self.texture_id().id());
        ui.invisible_button(title, ui.content_region_avail());

        // Get draw list and draw image over invisible button
        let draw_list = ui.get_window_draw_list();
        draw_list
            .add_image(self.texture_id, ui.item_rect_min(), ui.item_rect_max())
            .build();
    }

    pub fn render(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        self.update_camera(render_params);

        let title = format!("Texture {}", self.texture_id().id());
        let window = ui.window(title);

        let mut new_imgui_region_size = None;

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
        let (width, height) = render_params.viewport_size;
        if self.vp_size[0] != width as f32 || self.vp_size[1] != height as f32 {
            self.vp_size[0] = width as f32;
            self.vp_size[1] = height as f32;
            // Note: GpuCamera works in Imgui viewport
            let camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);
            self.camera = camera;

            let new_imgbuf = ImageBuffer::new(width, height);
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
                    *pixel = self.ray_color(x, y, render_params);
                }
            }
        }
    }

    /**
     *
     * Calculate the color of ray tracing, considering the followings:
     * 1. multitimes bouncing
     * 2. send ray from eye
     * 3. hit the sphere at, got intersection (point vector, normal vector, etc.)
     * 4. recursively send ray for sampling times with material color/texture, from p to unit sphere with normal vector lenght as radius
     * 5. convert normal plus other physical factors (attenuation, fuzzy refection) to get final color
     *
     * @params
     *
     * @ray:   the entre ray
     * @world: a impl Hittable, which can be hit by ray
     * @material: materials including metal, dielectric, lambertian, etc
     * @fuzzy:    fuzzy reflection factor
     * @depth: limit ray bouncing times
     */
    pub fn ray_color(
        &mut self,
        x: u32,
        y: u32,
        render_params: &RenderParams,
    ) -> Rgb<u8> {
        let [width, height] = self.vp_size;
        let u = coord_to_color(x, width);
        let v = coord_to_color(y, height);

        let n_samples = render_params.sampling.num_samples_per_pixel;
        let n_bounces = render_params.sampling.num_bounces;
        let mut depth: u32 = 20;
        let fuzzy = 0.9;

        let mut pixel_color = vec3(0.0, 0.0, 0.0);

        // sampleing
        for _s in 0..n_samples {
            let (uu, vv) = (u + random_f32(), v + random_f32());
            let ray = self.camera.make_ray(uu, vv);
            let mut metal_material = Metal {
                ray: &ray,
                albedo: vec3(1.0, 0.85, 0.57),
            };
            let mut grass_material = Scatter {
                ray: &ray,
                albedo: vec3(1.0, 0.85, 0.57),
            };
            let rec = Box::into_raw(Box::new(Intersection::new()));

            // if self.ray_hit_world(&ray, 0.001, f32::MAX, &mut rec) {
            if self.ray_hit_world_raw(&ray, self.world.clone(), 0.001, f32::MAX, rec) {
                if depth <= 0 {
                    return vec3_to_rgb8(vec3(0.0, 0.0, 0.0));
                }
                depth -= 1;
                // scatter + attenuation + reflect
                let (attenuation, scattered_ray) = metal_material.scatter_raw(rec);
                if self.ray_hit_world_raw(&scattered_ray, self.world.clone(), 0.001, f32::MAX, rec)
                {
                    unsafe {
                        let mut sampled_color = (*rec).n * 255.0 / 2.0;
                        sampled_color.x *= attenuation.x * fuzzy;
                        sampled_color.y *= attenuation.y * fuzzy;
                        sampled_color.z *= attenuation.z * fuzzy;

                        pixel_color += sampled_color;
                        return vec3_to_rgb8(pixel_color / 2.0);
                    }
                }
            };
        }

        vec3_to_rgb8(vec3(v * 255.0, u * 255.0, 255.0))
    }

    pub fn ray_hit_world(
        &mut self,
        ray: &Ray,
        tmin: f32,
        tmax: f32,
        rec: &mut Intersection,
    ) -> bool {
        let mut temp_rec = Intersection::new();
        let mut hit_anything = false;
        let mut closest_hit = tmax;
        let old_hit = rec.t;

        for object in self.world[..].into_iter() {
            let result = object.closest_hit(&ray, tmin, closest_hit, &mut temp_rec);
            if result.0 {
                hit_anything = true;
                closest_hit = old_hit;
                *rec = *(result.1.unwrap().deref_mut());
            }
        }

        return hit_anything;
    }

    pub fn ray_hit_world_raw(
        &mut self,
        ray: &Ray,
        world: Vec<Box<Sphere>>,
        tmin: f32,
        tmax: f32,
        rec: *mut Intersection,
    ) -> bool {
        unsafe {
            let mut temp_rec = Intersection::new();
            let mut hit_anything = false;
            let mut closest_hit = tmax;
            let old_hit = (*rec).t;

            for object in world[..].into_iter() {
                let result = object.closest_hit_2(&ray, tmin, closest_hit, &mut temp_rec);
                if result.0 {
                    hit_anything = true;
                    closest_hit = old_hit;
                    *rec = *(result.1.unwrap());
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
