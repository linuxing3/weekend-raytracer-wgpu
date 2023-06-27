#![deny(clippy::pedantic, nonstandard_style)]
use std::pin::Pin;

use super::{
    texture::*, GpuCamera, GpuMaterial, ImguiImage, ImguiRenderer, Material, RenderParams, Scene,
    Sphere,
};

use imgui::Ui;

// Layer trait/interface
pub trait Layer {
    fn on_attach(
        &mut self,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    );
    fn on_dettach(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        size: [f32; 2],
    );
    fn on_update(
        &mut self,
        dt: f32,
    );
    fn on_render(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    );
}

pub struct RayLayer {
    camera: GpuCamera,
    renderer: ImguiRenderer,
    scene: Scene,
    width: f32,
    height: f32,
    last_rendered_time: f32,
    material_data: Vec<GpuMaterial>,
    global_texture_data: Vec<[f32; 3]>,
}

impl Layer for RayLayer {
    fn on_attach(
        &mut self,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        unsafe {
            let image: Pin<&mut ImguiImage> = Pin::as_mut(&mut self.renderer.image);
            Pin::get_unchecked_mut(image).allocate_memory(device, queue, renderer);
        }
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
        dt: f32,
    ) {
        // self.camera.update;
    }

    fn on_render(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        // update image data from renderer
        self.render_data(ui, rp, device, queue, renderer);
        // render ui in layer
        self.render_ui(ui, rp);
    }

    // add code here
}

impl RayLayer {
    pub fn new(
        render_params: &RenderParams,
        camera: GpuCamera,
        dt: f32,
    ) -> Self {
        let scene = scene();

        // Note: GpuCamera works in Imgui viewport
        let camera_ptr = Box::into_raw(Box::new(camera));
        let global_texture_data = Vec::new();
        let material_data = Vec::with_capacity(0);

        let renderer = ImguiRenderer::new(render_params, camera_ptr);

        Self {
            renderer,
            camera,
            scene,
            width: 0.0,
            height: 0.0,
            last_rendered_time: dt,
            material_data,
            global_texture_data,
        }
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

    pub fn render_data(
        &mut self,
        ui: &mut Ui,
        rp: &RenderParams,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) {
        // set material and texture data in layer
        if self.set_global_data() {
            // call renderer to resize
            self.renderer
                .resize(self.width, self.height, device, queue, renderer);

            // call renderer to render
            let material_data_ptr = &self.material_data as *const Vec<GpuMaterial>;
            let global_texture_data_ptr = &self.global_texture_data as *const Vec<[f32; 3]>;
            self.renderer.render(
                rp,
                &mut self.camera,
                &mut self.scene,
                material_data_ptr,
                global_texture_data_ptr,
            );
        };
    }

    pub fn render_ui_draw_list(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        let image = &self.renderer.image;
        let title = format!("Texture {}", image.texture_id().id());

        ui.invisible_button(title, ui.content_region_avail());

        // Get draw list and draw image over invisible button
        let draw_list = ui.get_window_draw_list();

        draw_list
            .add_image(image.texture_id, ui.item_rect_min(), ui.item_rect_max())
            .build();
    }

    pub fn render_ui(
        &mut self,
        ui: &mut imgui::Ui,
        render_params: &RenderParams,
    ) {
        unsafe {
            let title = format!("Controller");
            let window = ui.window(title);

            let mut new_imgui_region_size = None;

            window
                .size([200.0, 200.0], imgui::Condition::FirstUseEver)
                .build(|| {
                    new_imgui_region_size = Some(ui.content_region_avail());
                    let sphere = &mut self.scene.spheres[0];

                    if ui.slider("z", -10.0, 10.0, &mut sphere.0.z) {};
                    if ui.slider("y", -10.0, 10.0, &mut sphere.0.y) {};
                    if ui.slider("x", -10.0, 10.0, &mut sphere.0.x) {};
                    if ui.slider("r", 0.0, 100.0, &mut sphere.1) {};

                    let image = &self.renderer.image;
                    imgui::Image::new(image.texture_id(), new_imgui_region_size.unwrap()).build(ui);
                });
        }
    }

    /// Returns a reference to the material data of this [`RayLayer`].
    pub fn material_data(&self) -> &Vec<GpuMaterial> {
        self.material_data.as_ref()
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
        Sphere::new(glm::vec3(0.0, -500.0, -1.0), 500.0, 0_u32),
        Sphere::new(glm::vec3(0.0, 1.0, 0.0), 1.0, 3_u32),
        Sphere::new(glm::vec3(-5.0, 1.0, 0.0), 1.0, 2_u32),
        Sphere::new(glm::vec3(5.0, 0.8, 1.5), 0.8, 1_u32),
        Sphere::new(glm::vec3(5.0, 1.2, -1.5), 1.2, 4_u32),
    ];

    Scene { spheres, materials }
}
