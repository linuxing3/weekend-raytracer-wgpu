use super::{
    math::*, texture::*, GpuCamera, Hittable, HittableV2, Intersection, Metal, Ray, RenderParams,
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

        let hit = &mut Intersection::new();
        // hittable world
        for object in &self.world {
            let mut pixel_color = Rgb([0_u8, 0_u8, 0_u8]);
            // NOTE:: multisampling
            // https://raytracing.github.io/images/fig-1.07-pixel-samples.jpg
            let n_samples = render_params.sampling.num_samples_per_pixel;

            for _s in 0..n_samples * 5 {
                let (uu, vv) = (u + random_f32(), v + random_f32());
                // NOTE: make ray from camera eye to sphere
                // https://raytracing.github.io/images/fig-1.04-ray-sphere.jpg
                let ray_from_camera = self.camera.make_ray(uu, vv);

                // HACK:
                let mut metal_material = Metal {
                    ray: &ray_from_camera,
                    albedo: vec3(1.0, 0.85, 0.57),
                };
                // material + fuzzy + multisampling
                let traced_color = ray_color_recursive_mat(
                    &ray_from_camera,
                    object,
                    &mut metal_material,
                    0.9,
                    50,
                    hit,
                );

                pixel_color = vec3_to_rgb8(adjust_gamma2_color(
                    rgb8_to_vec3(pixel_color) + rgb8_to_vec3(traced_color),
                    n_samples,
                ));
                return pixel_color;
            }
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
}

/**
 *
 * Calculate the color of ray tracing, considering the followings:
 * 1. multitimes bouncing
 * 2. send ray from eye
 * 3. hit the sphere at, got intersection (point vector, normal vector, etc.)
 * 4. resend ray from p to unit sphere with normal vector lenght as radius
 * 5. convert normal plus other physical factors to get final color
 *
 * @params
 *
 * @ray:   the entre ray
 * @world: a impl Hittable, which can be hit by ray
 * @depth: limit ray bouncing times
 */
fn ray_color(
    mut ray: &Ray,
    world: &impl Hittable,
    depth: u8,
) -> Rgb<u8> {
    let (_camera_root, camera_hit) = world.trace_ray(&ray, 0.0, num::Float::max_value());

    // NOTE: difussion
    // https://raytracing.github.io/images/fig-1.09-rand-vec.jpg
    let target = camera_hit.p + camera_hit.n + random_in_unit_sphere();
    let mut unit_ray_from_p = Ray::new(camera_hit.p, target - camera_hit.p);

    // NOTE:
    // make ray from camera-sphere hitting point
    // to some random point in the unit_normal_sphere
    let (_unit_root, unit_hit) =
        world.trace_ray(&mut unit_ray_from_p, 0.0, num::Float::max_value());
    let n_normal = unit_hit.n;

    let ray_color = rgb8_from_vec3([
        0.5 * (n_normal.x + 1.0),
        0.5 * (n_normal.y + 1.0),
        0.5 * (n_normal.z + 1.0),
    ]);
    let background_color = rgb8_from_vec3([n_normal.x * 0.5, n_normal.y * 0.5, n_normal.z * 0.5]);
    if camera_hit.t >= 0.0 {
        return ray_color;
    } else {
        return background_color;
    }
}

/**
 *
 * Calculate the color of ray tracing, considering the followings:
 * 1. multitimes bouncing
 * 2. send ray from eye
 * 3. hit the sphere at, got intersection (point vector, normal vector, etc.)
 * 4. recursively send ray for sampling times, from p to unit sphere with normal vector lenght as radius
 * 5. convert normal plus other physical factors to get final color
 *
 * @params
 *
 * @ray:   the entre ray
 * @world: a impl Hittable, which can be hit by ray
 * @depth: limit ray bouncing times
 */
fn ray_color_recursive(
    ray: &Ray,
    world: &impl Hittable,
    depth: u8,
) -> Rgb<u8> {
    if depth <= 0 {
        return Rgb([0, 0, 0]);
    };

    // lerp ray tracing color
    let (root, hit) = world.trace_ray(ray, 0.001, num::Float::max_value());

    if root >= 0.0 {
        // uniform scatter direction for all angles away from the hit point
        let target = hit.p + hit.n + random_in_hemisphere(hit.n);
        let unit_ray_from_p = Ray::new(hit.p, target - hit.p);
        return vec3_to_rgb8(
            0.5 * rgb8_to_vec3(ray_color_recursive(&unit_ray_from_p, world, depth - 1)),
        );
    }

    // lerp background color
    return default_background(ray);
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
fn ray_color_recursive_mat(
    ray: &Ray,
    object: &impl HittableV2,
    material: &mut impl Scatterable,
    fuzzy: f32,
    depth: u8,
    hit: &mut Intersection,
) -> Rgb<u8> {
    if depth <= 0 {
        return Rgb([0, 0, 0]);
    };

    // HACK: lerp ray tracing color, where hit.t = root = closest_t = moving step
    if object.trace_ray_v2(&ray, 0.001, hit.t, hit) {
        let (attenuation, scattered_ray) = material.scatter(&hit);
        let mut color_v = rgb8_to_vec3(ray_color_recursive_mat(
            &scattered_ray,
            object,
            material,
            fuzzy,
            depth - 1,
            hit,
        ));

        color_v.x *= attenuation.x * fuzzy;
        color_v.y *= attenuation.y * fuzzy;
        color_v.z *= attenuation.z * fuzzy;
        return vec3_to_rgb8(color_v);
    };

    // lerp background color
    return default_background(ray);
}

fn default_background(ray: &Ray) -> Rgb<u8> {
    let unit_direction = ray.direction.normalize();
    let t = 0.5 * (unit_direction.y + 1.0);
    let start_color_v3 = glm::vec3(1.0, 1.0, 1.0);
    let end_color_v3 = glm::vec3(0.5, 0.7, 1.0);
    let background_color_v3 = (1.0 - t) * start_color_v3 + t * end_color_v3;
    let background_color = vec3_to_rgb8(255.0 * background_color_v3);
    background_color
}
