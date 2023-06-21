use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;
use std::{borrow::Borrow, ops::DerefMut, ptr::null_mut};

use super::{
    math::*, scatter_lambertian, scatter_metal, texture::*, texture_lookup, GpuCamera, GpuMaterial,
    Intersection, Material, Metal, Ray, RenderParams, Scatterable, Scene, Sphere,
    TextureDescriptor,
};

use image::{DynamicImage, ImageBuffer, Rgb};
use imgui::{TextureId, Ui};
use nalgebra_glm::{dot, normalize, vec3, Vec3};

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
        // let imgbuf = Box::into_raw(Box::new(new_buffer));
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

    // BUG:
    pub fn allocate_memory(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
        size: wgpu::Extent3d,
    ) {
        unsafe {
            let img = DynamicImage::from(self.imgbuf.clone());
            let bytes: &[u8] = &img.to_rgba8();
            let imgui_texture =
                WgpuTexture::new_imgui_texture(&device, &queue, &renderer, bytes, size);

            self.texture_id = renderer.textures.insert(imgui_texture);
        }
    }

    pub fn set_imgbuf(
        &mut self,
        imgbuf: &mut XImageBuffer,
    ) {
        unsafe {
            self.imgbuf_pin = NonNull::from(imgbuf);
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
        if self.width == w && self.height == h {
            ()
        }
        self.width = w;
        self.height = h;
        self.release();
        let size = wgpu::Extent3d {
            width: self.width as u32,
            height: self.height as u32,
            depth_or_array_layers: 1,
        };
        self.allocate_memory(device, queue, renderer, size);
    }

    pub fn release(&mut self) {
        // self.imgbuf = null_mut();
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

// Layer trait/interface
pub trait Layer {
    fn on_attach(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    );
    fn on_dettach(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    );
    fn on_update(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    );
    fn on_render(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
    );
}

pub struct RayLayer {
    camera: GpuCamera,
    pub renderer: ImguiRenderer,
    scene: Scene,
    width: f32,
    height: f32,
    pub last_rendered_time: f32,
    material_data: Vec<GpuMaterial>,
    global_texture_data: Vec<[f32; 3]>,
}

impl Layer for RayLayer {
    fn on_attach(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    ) {
    }
    fn on_dettach(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    ) {
    }

    fn on_update(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    ) {
        // self.camera.update;
    }

    fn on_render(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
    ) {
        self.render_image(ui, rp);
        self.render_controller(ui, rp);
    }

    // add code here
}

impl RayLayer {
    pub fn new(
        render_params: &RenderParams,
        camera: GpuCamera,
    ) -> Self {
        let camera_ptr = Box::into_raw(Box::new(camera));
        let renderer = ImguiRenderer::new(render_params, camera_ptr);
        // Note: GpuCamera works in Imgui viewport
        let scene = Self::scene();

        // let world = scene.spheres[..]
        //     .into_iter()
        //     .map(move |s| Box::new(s.clone()))
        //     .collect();
        //
        // let materials = scene.materials;
        //
        let global_texture_data: Vec<[f32; 3]> = Vec::new();
        //
        let material_data: Vec<GpuMaterial> = Vec::with_capacity(0);

        Self {
            renderer,
            camera,
            scene,
            width: 0.0,
            height: 0.0,
            last_rendered_time: 0.0,
            material_data,
            global_texture_data,
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
            // Sphere::new(glm::vec3(0.0, 1.0, 0.0), 1.0, 3_u32),
            // Sphere::new(glm::vec3(-5.0, 1.0, 0.0), 1.0, 2_u32),
            // Sphere::new(glm::vec3(2.0, -1.0, 0.0), 2.0, 3_u32),
            // Sphere::new(glm::vec3(5.0, 0.8, 1.5), 0.8, 1_u32),
        ];

        Scene { spheres, materials }
    }
    pub fn set_global_data(&mut self) -> bool {
        self.material_data = Vec::with_capacity(self.scene.materials.len());

        for material in self.scene.materials.iter() {
            let gpu_material = match material {
                Material::Lambertian { albedo } => {
                    GpuMaterial::lambertian(albedo, &mut self.global_texture_data)
                }
                Material::Metal { albedo, fuzz } => {
                    GpuMaterial::metal(albedo, *fuzz, &mut self.global_texture_data)
                }
                Material::Dielectric { refraction_index } => {
                    GpuMaterial::dielectric(*refraction_index)
                }
                Material::Checkerboard { odd, even } => {
                    GpuMaterial::checkerboard(odd, even, &mut self.global_texture_data)
                }
            };

            self.material_data.push(gpu_material);
        }

        true
    }

    pub fn render(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        self.renderer
            .resize(self.width, self.height, device, queue, renderer);
        self.renderer.render(rp, &mut self.camera, &mut self.scene);
    }

    pub fn render_image(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        unsafe {
            let image = &self.renderer.image;
            imgui::Image::new(image.texture_id(), [image.width(), image.height()]).build(ui);
        }
    }

    pub fn render_controller(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        unsafe {
            let title = format!("Controller");
            let window = ui.window(title);

            window
                .size([200.0, 200.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    let sphere = &mut self.scene.spheres[0].clone();

                    if ui.slider("x", -10.0, 10.0, &mut sphere.0.x) {};
                    if ui.slider("y", -10.0, 10.0, &mut sphere.0.y) {};
                    if ui.slider("z", -10.0, 10.0, &mut sphere.0.z) {};

                    let image = &self.renderer.image;
                    imgui::Image::new(image.texture_id(), [image.width(), image.height()])
                        .build(ui);
                });
        }
    }
}

pub struct ImguiRenderer {
    pub image: Pin<Box<ImguiImage>>,
    pub camera: *mut GpuCamera,
    pub scene: *mut Scene,
    pub image_data: *mut XImageBuffer,
}

impl ImguiRenderer {
    pub fn new(
        render_params: &RenderParams,
        camera: *mut GpuCamera,
    ) -> Self {
        let mut image = ImguiImage::new(100.0, 100.0);
        Self {
            camera,
            image,
            scene: null_mut(),
            image_data: null_mut(),
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
            let image = &self.image;
            let height = image.height();
            let width = image.width();
            if height == h && width == w {
                ()
            }
            // image.resize(w, h, device, queue, renderer);
            self.image_data = null_mut();
            let new_buffer: XImageBuffer = ImageBuffer::new(width as u32, height as u32);
            self.image_data = Box::into_raw(Box::new(new_buffer));
        }
    }

    fn render(
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
