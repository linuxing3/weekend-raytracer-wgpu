use super::{
    math::*, texture::*, GpuCamera, HittableWorld, Intersection, Metal, Ray, RenderParams,
    Scatterable, Sphere,
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
    pub world: Vec<Sphere>,
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
        let mut world = vec![];

        for i in 0..5 {
            world.push(Sphere::new(
                glm::vec3(-3.0 * (i as f32), 1.0 * (i as f32), 0.0 + (i as f32)),
                1.0,
                2,
            ));
        }

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
                    *pixel = self.per_pixel(x, y, render_params);
                }
            }
        }
    }

    // BUG:
    pub fn per_pixel(
        &mut self,
        x: u32,
        y: u32,
        render_params: &RenderParams,
    ) -> Rgb<u8> {
        let [width, height] = self.vp_size;
        let u = coord_to_color(x, width);
        let v = coord_to_color(y, height);
        let (uu, vv) = (u + random_f32(), v + random_f32());
        let ray = self.camera.make_ray(uu, vv);

        let n_samples = render_params.sampling.num_samples_per_pixel;
        let n_bounces = render_params.sampling.num_bounces;

        let mut metal_material = Metal {
            ray: &ray,
            albedo: vec3(1.0, 0.85, 0.57),
        };
        let test_hit = &mut Intersection::new();
        for _b in 0..n_bounces {
            return self.trace_ray_color(&ray, n_samples, &mut metal_material, test_hit);
        }

        // when world is empty
        vec3_to_rgb8(glm::vec3(0.5, 0.7, 1.0))
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

    pub fn miss_hit(
        &self,
        _ray: &Ray,
    ) -> Intersection {
        let mut closest_hit = Intersection::new();
        closest_hit.t = -1.0;
        return closest_hit;
    }

    pub fn closest_hit(
        &self,
        ray: &Ray,
        root: f32,
        object_index: usize,
    ) -> Intersection {
        let mut closest_hit = Intersection::new();
        closest_hit.t = root;

        let closest_object = self.world.as_slice()[object_index];

        let new_origin = ray.origin - closest_object.center.xyz();
        closest_hit.p = new_origin + ray.direction * root;
        // HACK:
        closest_hit.n = glm::normalize(&closest_hit.p);

        closest_hit.p += closest_object.center.xyz();

        return closest_hit;
    }
}
