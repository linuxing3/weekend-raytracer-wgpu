pub use color::Color;
use gpu_buffer::{StorageBuffer, UniformBuffer};
use image::Rgb;
pub use math::*;
use nalgebra_glm::{acos, atan2, dot, vec3, Vec3};
use wgpu::util::DeviceExt;
pub use {angle::Angle, layer::Layer, texture::Texture, texture::WgpuTexture};

use thiserror::Error;

mod angle;
mod color;
mod gpu_buffer;
mod layer;
mod math;
mod texture;

use std::f32::consts::*;

pub struct Raytracer {
    vertex_uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    frame_data_buffer: UniformBuffer,
    image_bind_group: wgpu::BindGroup,
    camera_buffer: UniformBuffer,
    sampling_parameter_buffer: UniformBuffer,
    hw_sky_state_buffer: StorageBuffer,
    parameter_bind_group: wgpu::BindGroup,
    scene_bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    latest_render_params: RenderParams,
    render_progress: RenderProgress,
    frame_number: u32,
}

impl Raytracer {
    pub fn new(
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        scene: &Scene,
        render_params: &RenderParams,
        max_viewport_resolution: u32,
    ) -> Result<Self, RenderParamsValidationError> {
        match render_params.validate() {
            Ok(_) => {}
            Err(err) => return Err(err),
        }

        let uniforms = VertexUniforms {
            view_projection_matrix: unit_quad_projection_matrix(),
            model_matrix: glm::identity(),
        };

        let vertex_uniform_buffer = UniformBuffer::new_from_bytes(
            device,
            bytemuck::bytes_of(&uniforms),
            0_u32,
            Some("uniforms"),
        );

        let vertex_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[vertex_uniform_buffer.layout(wgpu::ShaderStages::VERTEX)],
                label: Some("uniforms layout"),
            });

        let vertex_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &vertex_uniform_bind_group_layout,
            entries: &[vertex_uniform_buffer.binding()],
            label: Some("uniforms bind group"),
        });

        let frame_data_buffer =
            UniformBuffer::new(device, 16_u64, 0_u32, Some("frame data buffer"));

        let image_buffer = {
            let buffer = vec![[0_f32; 3]; max_viewport_resolution as usize];

            StorageBuffer::new_from_bytes(
                device,
                bytemuck::cast_slice(buffer.as_slice()),
                1_u32,
                Some("image buffer"),
            )
        };

        let image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    frame_data_buffer.layout(wgpu::ShaderStages::FRAGMENT),
                    image_buffer.layout(wgpu::ShaderStages::FRAGMENT, false),
                ],
                label: Some("image layout"),
            });

        let image_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &image_bind_group_layout,
            entries: &[frame_data_buffer.binding(), image_buffer.binding()],
            label: Some("image bind group"),
        });

        let camera_buffer = {
            let camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);

            UniformBuffer::new_from_bytes(
                device,
                bytemuck::bytes_of(&camera),
                0_u32,
                Some("camera buffer"),
            )
        };

        let sampling_parameter_buffer = UniformBuffer::new(
            device,
            std::mem::size_of::<GpuSamplingParams>() as wgpu::BufferAddress,
            1_u32,
            Some("sampling parameter buffer"),
        );

        let hw_sky_state_buffer = {
            let sky_state = render_params.sky.to_sky_state()?;

            StorageBuffer::new_from_bytes(
                device,
                bytemuck::bytes_of(&sky_state),
                2_u32,
                Some("sky state buffer"),
            )
        };

        let parameter_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    camera_buffer.layout(wgpu::ShaderStages::FRAGMENT),
                    sampling_parameter_buffer.layout(wgpu::ShaderStages::FRAGMENT),
                    hw_sky_state_buffer.layout(wgpu::ShaderStages::FRAGMENT, true),
                ],
                label: Some("parameter layout"),
            });

        let parameter_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &parameter_bind_group_layout,
            entries: &[
                camera_buffer.binding(),
                sampling_parameter_buffer.binding(),
                hw_sky_state_buffer.binding(),
            ],
            label: Some("parameter bind group"),
        });

        let (scene_bind_group_layout, scene_bind_group) = {
            let sphere_buffer = StorageBuffer::new_from_bytes(
                device,
                bytemuck::cast_slice(scene.spheres.as_slice()),
                0_u32,
                Some("scene buffer"),
            );

            let mut global_texture_data: Vec<[f32; 3]> = Vec::new();

            let mut material_data: Vec<GpuMaterial> = Vec::with_capacity(scene.materials.len());

            for material in scene.materials.iter() {
                let gpu_material = match material {
                    Material::Lambertian { albedo } => {
                        GpuMaterial::lambertian(albedo, &mut global_texture_data)
                    }
                    Material::Metal { albedo, fuzz } => {
                        GpuMaterial::metal(albedo, *fuzz, &mut global_texture_data)
                    }
                    Material::Dielectric { refraction_index } => {
                        GpuMaterial::dielectric(*refraction_index)
                    }
                    Material::Checkerboard { odd, even } => {
                        GpuMaterial::checkerboard(odd, even, &mut global_texture_data)
                    }
                };

                material_data.push(gpu_material);
            }

            let material_buffer = StorageBuffer::new_from_bytes(
                device,
                bytemuck::cast_slice(material_data.as_slice()),
                1_u32,
                Some("materials buffer"),
            );

            let texture_buffer = StorageBuffer::new_from_bytes(
                device,
                bytemuck::cast_slice(global_texture_data.as_slice()),
                2_u32,
                Some("textures buffer"),
            );

            let scene_bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        sphere_buffer.layout(wgpu::ShaderStages::FRAGMENT, true),
                        material_buffer.layout(wgpu::ShaderStages::FRAGMENT, true),
                        texture_buffer.layout(wgpu::ShaderStages::FRAGMENT, true),
                    ],
                    label: Some("scene layout"),
                });

            let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &scene_bind_group_layout,
                entries: &[
                    sphere_buffer.binding(),
                    material_buffer.binding(),
                    texture_buffer.binding(),
                ],
                label: Some("scene bind group"),
            });

            (scene_bind_group_layout, scene_bind_group)
        };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("raytracer.wgsl").into()),
            label: Some("raytracer.wgsl"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[
                &vertex_uniform_bind_group_layout,
                &image_bind_group_layout,
                &parameter_bind_group_layout,
                &scene_bind_group_layout,
            ],
            push_constant_ranges: &[],
            label: Some("raytracer layout"),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vsMain",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fsMain",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                polygon_mode: wgpu::PolygonMode::Fill,
                cull_mode: Some(wgpu::Face::Back),
                // Requires Features::DEPTH_CLAMPING
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            label: Some("raytracer pipeline"),
            // If the pipeline will be used with a multiview render pass, this
            // indicates how many array layers the attachments will have.
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
            label: Some("VertexInput buffer"),
        });

        let render_progress = RenderProgress::new();

        let frame_number = 1_u32;

        Ok(Self {
            vertex_uniform_bind_group,
            frame_data_buffer,
            image_bind_group,
            camera_buffer,
            sampling_parameter_buffer,
            hw_sky_state_buffer,
            parameter_bind_group,
            scene_bind_group,
            vertex_buffer,
            pipeline,
            latest_render_params: *render_params,
            render_progress,
            frame_number,
        })
    }

    pub fn render_frame<'a>(
        &'a mut self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        {
            let gpu_sampling_params = self
                .render_progress
                .next_frame(&self.latest_render_params.sampling);

            queue.write_buffer(
                &self.sampling_parameter_buffer.handle(),
                0,
                bytemuck::cast_slice(&[gpu_sampling_params]),
            );
        }

        {
            let viewport_size = self.latest_render_params.viewport_size;

            let frame_number = self.frame_number;

            let frame_data = [viewport_size.0, viewport_size.1, frame_number];

            queue.write_buffer(
                &self.frame_data_buffer.handle(),
                0,
                bytemuck::cast_slice(&frame_data),
            );
        }

        render_pass.set_pipeline(&self.pipeline);

        render_pass.set_bind_group(0, &self.vertex_uniform_bind_group, &[]);

        render_pass.set_bind_group(1, &self.image_bind_group, &[]);

        render_pass.set_bind_group(2, &self.parameter_bind_group, &[]);

        render_pass.set_bind_group(3, &self.scene_bind_group, &[]);

        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        let num_vertices = VERTICES.len() as u32;

        render_pass.draw(0..num_vertices, 0..1);

        self.frame_number += 1_u32;
    }

    pub fn set_render_params(
        &mut self,
        queue: &wgpu::Queue,
        render_params: &RenderParams,
    ) -> Result<(), RenderParamsValidationError> {
        if *render_params == self.latest_render_params {
            return Ok(());
        }

        match render_params.validate() {
            Ok(_) => {}
            Err(err) => return Err(err),
        }

        {
            let sky_state = render_params.sky.to_sky_state()?;

            queue.write_buffer(
                &self.hw_sky_state_buffer.handle(),
                0,
                bytemuck::bytes_of(&sky_state),
            );
        }

        {
            let camera = GpuCamera::new(&render_params.camera, render_params.viewport_size);

            queue.write_buffer(&self.camera_buffer.handle(), 0, bytemuck::bytes_of(&camera));
        }

        self.latest_render_params = *render_params;

        self.render_progress.reset();

        Ok(())
    }

    pub fn progress(&self) -> f32 {
        self.render_progress.accumulated_samples() as f32
            / self.latest_render_params.sampling.max_samples_per_pixel as f32
    }
}

#[derive(Error, Debug)]

pub enum RenderParamsValidationError {
    #[error("max_samples_per_pixel ({0}) is not a multiple of num_samples_per_pixel ({1})")]
    MaxSampleCountNotMultiple(u32, u32),
    #[error("viewport_size elements cannot be zero: ({0}, {1})")]
    ViewportSize(u32, u32),
    #[error("vfov must be between 0..=90 degrees")]
    VfovOutOfRange(f32),
    #[error("aperture must be between 0..=1")]
    ApertureOutOfRange(f32),
    #[error("focus_distance must be greater than zero")]
    FocusDistanceOutOfRange(f32),
    #[error(transparent)]
    HwSkyModelValidationError(#[from] hw_skymodel::rgb::Error),
}

pub struct Scene {
    pub spheres: Vec<Sphere>,
    pub materials: Vec<Material>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

pub struct Sphere(glm::Vec4, f32, u32, [u32; 2]);

impl Sphere {
    pub fn new(
        center: glm::Vec3,
        radius: f32,
        material_idx: u32,
    ) -> Self {
        Self(glm::vec3_to_vec4(&center), radius, material_idx, [0_u32; 2])
    }
}

pub enum Material {
    Lambertian { albedo: Texture },
    Metal { albedo: Texture, fuzz: f32 },
    Dielectric { refraction_index: f32 },
    Checkerboard { even: Texture, odd: Texture },
}

#[derive(Clone, Copy, PartialEq)]

pub struct RenderParams {
    pub camera: Camera,
    pub sky: SkyParams,
    pub sampling: SamplingParams,
    pub viewport_size: (u32, u32),
}

impl RenderParams {
    fn validate(&self) -> Result<(), RenderParamsValidationError> {
        if self.sampling.max_samples_per_pixel % self.sampling.num_samples_per_pixel != 0 {
            return Err(RenderParamsValidationError::MaxSampleCountNotMultiple(
                self.sampling.max_samples_per_pixel,
                self.sampling.num_samples_per_pixel,
            ));
        }

        if self.viewport_size.0 == 0_u32 || self.viewport_size.1 == 0_u32 {
            return Err(RenderParamsValidationError::ViewportSize(
                self.viewport_size.0,
                self.viewport_size.1,
            ));
        }

        if !(Angle::degrees(0.0)..=Angle::degrees(90.0)).contains(&self.camera.vfov) {
            return Err(RenderParamsValidationError::VfovOutOfRange(
                self.camera.vfov.as_degrees(),
            ));
        }

        if !(0.0..=1.0).contains(&self.camera.aperture) {
            return Err(RenderParamsValidationError::ApertureOutOfRange(
                self.camera.aperture,
            ));
        }

        if self.camera.focus_distance < 0.0 {
            return Err(RenderParamsValidationError::FocusDistanceOutOfRange(
                self.camera.focus_distance,
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq)]

pub struct Camera {
    pub eye_pos: glm::Vec3,
    pub eye_dir: glm::Vec3,
    pub up: glm::Vec3,
    /// Angle must be between 0..=90 degrees.
    pub vfov: Angle,
    /// Aperture must be between 0..=1.
    pub aperture: f32,
    /// Focus distance must be a positive number.
    pub focus_distance: f32,
}

impl Camera {
    pub fn new() -> Self {
        let eye_pos = glm::vec3(0.0, 0.0, 2.0);

        let look_at = glm::vec3(0.0, 0.0, -1.0);

        let focus_distance = glm::magnitude(&(look_at - eye_pos));

        // camera orientation
        let yaw = Angle::degrees(0_f32);

        let pitch = Angle::degrees(0_f32);

        let forward = glm::vec3(
            yaw.as_radians().cos() * pitch.as_radians().cos(),
            pitch.as_radians().sin(),
            yaw.as_radians().sin() * pitch.as_radians().cos(),
        );

        let eye_dir = glm::normalize(&forward);

        let world_up = glm::vec3(0.0, 1.0, 0.0);

        let right = glm::cross(&eye_dir, &world_up);

        let up = glm::cross(&right, &eye_dir);

        let vfov_degrees = 30.0;

        let aperture = 0.8;

        Camera {
            eye_pos,
            eye_dir,
            up,
            vfov: Angle::degrees(vfov_degrees),
            aperture,
            focus_distance,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]

pub struct SkyParams {
    // Azimuth must be between 0..=360 degrees
    pub azimuth_degrees: f32,
    // Inclination must be between 0..=90 degrees
    pub zenith_degrees: f32,
    // Turbidity must be between 1..=10
    pub turbidity: f32,
    // Albedo elements must be between 0..=1
    pub albedo: [f32; 3],
}

impl Default for SkyParams {
    fn default() -> Self {
        Self {
            azimuth_degrees: 0_f32,
            zenith_degrees: 85_f32,
            turbidity: 4_f32,
            albedo: [1_f32; 3],
        }
    }
}

impl SkyParams {
    fn to_sky_state(self: &SkyParams) -> Result<GpuSkyState, hw_skymodel::rgb::Error> {
        let azimuth = Angle::degrees(self.azimuth_degrees).as_radians();

        let zenith = Angle::degrees(self.zenith_degrees).as_radians();

        let sun_direction = [
            zenith.sin() * azimuth.cos(),
            zenith.cos(),
            zenith.sin() * azimuth.sin(),
            0_f32,
        ];

        let state = hw_skymodel::rgb::SkyState::new(&hw_skymodel::rgb::SkyParams {
            elevation: FRAC_PI_2 - zenith,
            turbidity: self.turbidity,
            albedo: self.albedo,
        })?;

        let (params_data, radiance_data) = state.raw();

        Ok(GpuSkyState {
            params: params_data,
            radiances: radiance_data,
            _padding: [0_u32, 2],
            sun_direction,
        })
    }
}

#[derive(Clone, Copy, PartialEq)]

pub struct SamplingParams {
    pub max_samples_per_pixel: u32,
    pub num_samples_per_pixel: u32,
    pub num_bounces: u32,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            max_samples_per_pixel: 128_u32,
            num_samples_per_pixel: 2_u32,
            num_bounces: 8_u32,
        }
    }
}

struct RenderProgress {
    accumulated_samples_per_pixel: u32,
}

impl RenderProgress {
    pub fn new() -> Self {
        Self {
            accumulated_samples_per_pixel: 0_u32,
        }
    }

    pub fn next_frame(
        &mut self,
        sampling_params: &SamplingParams,
    ) -> GpuSamplingParams {
        let current_accumulated_samples = self.accumulated_samples_per_pixel;

        let next_accumulated_samples =
            sampling_params.num_samples_per_pixel + current_accumulated_samples;

        // Initial state: no samples have been accumulated yet. This is the first frame
        // after a reset. The image buffer's previous samples should be cleared by
        // setting clear_accumulated_samples to 1_u32.
        if current_accumulated_samples == 0_u32 {
            self.accumulated_samples_per_pixel = next_accumulated_samples;

            GpuSamplingParams {
                num_samples_per_pixel: sampling_params.num_samples_per_pixel,
                num_bounces: sampling_params.num_bounces,
                accumulated_samples_per_pixel: next_accumulated_samples,
                clear_accumulated_samples: 1_u32,
            }
        }
        // Progressive render: accumulating samples in the image buffer over multiple
        // frames.
        else if next_accumulated_samples <= sampling_params.max_samples_per_pixel {
            self.accumulated_samples_per_pixel = next_accumulated_samples;

            GpuSamplingParams {
                num_samples_per_pixel: sampling_params.num_samples_per_pixel,
                num_bounces: sampling_params.num_bounces,
                accumulated_samples_per_pixel: next_accumulated_samples,
                clear_accumulated_samples: 0_u32,
            }
        }
        // Completed render: we have accumulated max_samples_per_pixel samples. Stop rendering
        // by setting num_samples_per_pixel to zero.
        else {
            GpuSamplingParams {
                num_samples_per_pixel: 0_u32,
                num_bounces: sampling_params.num_bounces,
                accumulated_samples_per_pixel: current_accumulated_samples,
                clear_accumulated_samples: 0_u32,
            }
        }
    }

    pub fn reset(&mut self) {
        self.accumulated_samples_per_pixel = 0_u32;
    }

    pub fn accumulated_samples(&self) -> u32 {
        self.accumulated_samples_per_pixel
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

pub struct GpuCamera {
    eye: glm::Vec3,
    _padding1: f32,
    horizontal: glm::Vec3,
    _padding2: f32,
    vertical: glm::Vec3,
    _padding3: f32,
    u: glm::Vec3,
    _padding4: f32,
    v: glm::Vec3,
    lens_radius: f32,
    lower_left_corner: glm::Vec3,
    _padding5: f32,
}

impl GpuCamera {
    pub fn new(
        camera: &Camera,
        viewport_size: (u32, u32),
    ) -> Self {
        let lens_radius = 0.5_f32 * camera.aperture;

        let aspect = viewport_size.0 as f32 / viewport_size.1 as f32;

        let theta = camera.vfov.as_radians();

        let half_height = camera.focus_distance * (0.5_f32 * theta).tan();

        let half_width = aspect * half_height;

        let w = glm::normalize(&camera.eye_dir);

        let v = glm::normalize(&camera.up);

        let u = glm::cross(&w, &v);

        let lower_left_corner =
            camera.eye_pos + camera.focus_distance * w - half_width * u - half_height * v;

        let horizontal = 2_f32 * half_width * u;

        let vertical = 2_f32 * half_height * v;

        Self {
            eye: camera.eye_pos,
            _padding1: 0_f32,
            horizontal,
            _padding2: 0_f32,
            vertical,
            _padding3: 0_f32,
            u,
            _padding4: 0_f32,
            v,
            lens_radius,
            lower_left_corner,
            _padding5: 0_f32,
        }
    }

    // NOTE: make ray fro camera
    // https://raytracing.github.io/images/fig-1.03-cam-geom.jpg
    pub fn make_ray(
        &mut self,
        u: f32,
        v: f32,
    ) -> Ray {
        Ray::new(
            self.eye,
            self.lower_left_corner + u * self.horizontal + v * self.vertical - self.eye,
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

struct GpuMaterial {
    id: u32,
    desc1: TextureDescriptor,
    desc2: TextureDescriptor,
    x: f32,
}

impl GpuMaterial {
    pub fn lambertian(
        albedo: &Texture,
        global_texture_data: &mut Vec<[f32; 3]>,
    ) -> Self {
        Self {
            id: 0_u32,
            desc1: Self::append_to_global_texture_data(albedo, global_texture_data),
            desc2: TextureDescriptor::empty(),
            x: 0_f32,
        }
    }

    pub fn metal(
        albedo: &Texture,
        fuzz: f32,
        global_texture_data: &mut Vec<[f32; 3]>,
    ) -> Self {
        Self {
            id: 1_u32,
            desc1: Self::append_to_global_texture_data(albedo, global_texture_data),
            desc2: TextureDescriptor::empty(),
            x: fuzz,
        }
    }

    pub fn dielectric(refraction_index: f32) -> Self {
        Self {
            id: 2_u32,
            desc1: TextureDescriptor::empty(),
            desc2: TextureDescriptor::empty(),
            x: refraction_index,
        }
    }

    pub fn checkerboard(
        even: &Texture,
        odd: &Texture,
        global_texture_data: &mut Vec<[f32; 3]>,
    ) -> Self {
        Self {
            id: 3_u32,
            desc1: Self::append_to_global_texture_data(even, global_texture_data),
            desc2: Self::append_to_global_texture_data(odd, global_texture_data),
            x: 0_f32,
        }
    }

    fn append_to_global_texture_data(
        texture: &Texture,
        global_texture_data: &mut Vec<[f32; 3]>,
    ) -> TextureDescriptor {
        let dimensions = texture.dimensions();

        let offset = global_texture_data.len() as u32;

        global_texture_data.extend_from_slice(texture.as_slice());

        TextureDescriptor {
            width: dimensions.0,
            height: dimensions.1,
            offset,
        }
    }

    pub fn register_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
        texture: &Texture,
        global_texture_data: &mut Vec<[f32; 3]>,
    ) -> Option<imgui::TextureId> {
        let bytes: Vec<u8> = global_texture_data
            .as_slice()
            .into_iter()
            .map(|p| -> Vec<u8> {
                vec![
                    (255.0 * p[0]) as u8,
                    (255.0 * p[1]) as u8,
                    (255.0 * p[2]) as u8,
                ]
            })
            .flatten()
            .collect::<_>();

        let (width, height) = texture.dimensions();

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let imgui_texture =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        let texture_id = renderer.textures.insert(imgui_texture);

        Some(texture_id)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

pub struct TextureDescriptor {
    width: u32,
    height: u32,
    offset: u32,
}

impl TextureDescriptor {
    pub fn empty() -> Self {
        Self {
            width: 0_u32,
            height: 0_u32,
            offset: 0xffffffff,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

struct GpuSkyState {
    params: [f32; 27],       // 0 byte offset, 108 byte size
    radiances: [f32; 3],     // 108 byte offset, 12 byte size
    _padding: [u32; 2],      // 120 byte offset, 8 byte size
    sun_direction: [f32; 4], // 128 byte offset, 16 byte size
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

struct GpuSamplingParams {
    num_samples_per_pixel: u32,
    num_bounces: u32,
    accumulated_samples_per_pixel: u32,
    clear_accumulated_samples: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]

struct VertexUniforms {
    view_projection_matrix: glm::Mat4,
    model_matrix: glm::Mat4,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]

struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // @location(0)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                // @location(1)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                },
            ],
        }
    }
}

fn unit_quad_projection_matrix() -> glm::Mat4 {
    let sw = 0.5_f32;

    let sh = 0.5_f32;

    // Our ortho camera is just centered at (0, 0)

    let left = -sw;

    let right = sw;

    let bottom = -sh;

    let top = sh;

    // DirectX, Metal, wgpu share the same left-handed coordinate system
    // for their normalized device coordinates:
    // https://github.com/gfx-rs/gfx/tree/master/src/backend/dx12
    glm::ortho_lh_zo(left, right, bottom, top, -1_f32, 1_f32)
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [-0.5, 0.5],
        tex_coords: [0.0, 0.0],
    },
    Vertex {
        position: [-0.5, -0.5],
        tex_coords: [0.0, 1.0],
    },
    Vertex {
        position: [0.5, -0.5],
        tex_coords: [1.0, 1.0],
    },
    Vertex {
        position: [-0.5, 0.5],
        tex_coords: [0.0, 0.0],
    },
    Vertex {
        position: [0.5, -0.5],
        tex_coords: [1.0, 1.0],
    },
    Vertex {
        position: [0.5, 0.5],
        tex_coords: [1.0, 0.0],
    },
];

// from wgsl
/**
 * lookup a per pixel texture from global textures vector
 */

pub fn texture_lookup(
    desc: TextureDescriptor,
    textures: &[[f32; 3]],
    u: f32,
    v: f32,
) -> Vec3 {
    let u = clamp(u, 0_f32, 1_f32);

    let v = 1_f32 - clamp(v, 0_f32, 1_f32);

    let j = (u * desc.width as f32) as u32;

    let i = (v * desc.height as f32) as u32;

    let idx = i * desc.width + j;

    let elem = (*textures)[desc.offset as usize + idx as usize];

    return vec3(elem[0], elem[1], elem[2]);
}

pub struct Ray {
    origin: Vec3,
    direction: Vec3,
}

impl Default for Ray {
    fn default() -> Self {
        let origin = glm::vec3(1.5, 1.5, -4.0);

        let direction = glm::vec3(0.0, 0.0, -1.0);

        Self { origin, direction }
    }
}

impl Ray {
    pub fn new(
        origin: Vec3,
        direction: Vec3,
    ) -> Self {
        Self { origin, direction }
    }

    pub fn new_from_xy(
        x: f32,
        y: f32,
    ) -> Self {
        let origin = glm::vec3(0.0, 0.0, 2.0);

        let direction = origin - glm::vec3(x, y, -1.0);

        Self { origin, direction }
    }
}

#[derive(Clone, Copy)]

pub struct Intersection {
    p: Vec3, //point
    n: Vec3, //normal
    u: f32,  // coordinate x
    v: f32,  // coordinate y
    t: f32,  // ray time
    f: bool, // front_face
    m: u32,  // material
}

impl Intersection {
    pub fn new() -> Self {
        let p: Vec3 = glm::vec3(0.0, 0.0, 0.0);

        let n: Vec3 = glm::vec3(0.0, 0.0, 0.0);

        let u: f32 = 0.0;

        let v: f32 = 0.0;

        let t: f32 = std::f32::MAX;

        let f: bool = false;

        let m: u32 = 0_u32;

        Self {
            p,
            n,
            u,
            v,
            t,
            f,
            m,
        }
    }

    pub fn set_face_normal(
        &mut self,
        ray: &Ray,
        outward_normal: Vec3,
    ) {
        self.f = glm::dot(&ray.direction, &outward_normal) < 0.0;

        match self.f {
            true => {
                self.n = outward_normal;
            }
            false => {
                self.n = -outward_normal;
            }
        }
    }
}

// implementation of sphere
impl Sphere {
    pub fn material_idx(&self) -> u32 {
        return self.2;
    }
}

impl Sphere {
    pub fn closest_hit_raw<'a>(
        &'a self,
        ray: &Ray,
        tmin: f32,
        tmax: f32,
        rec: *mut Intersection,
    ) -> (bool, Option<*mut Intersection>) {
        unsafe {
            let oc = ray.origin - self.0.xyz();

            let a = dot(&ray.direction, &ray.direction);

            let half_b = dot(&oc, &ray.direction);

            let c = dot(&oc, &oc) - self.1 * self.1;

            let discriminant = half_b * half_b - a * c;

            if discriminant < 0.0 {
                return (false, None);
            }

            let mut closest_t = (-half_b - num::Float::sqrt(discriminant)) / a;

            if closest_t < tmin || tmax < closest_t {
                closest_t = (-half_b + num::Float::sqrt(discriminant)) / a;

                if closest_t < tmin || tmax < closest_t {
                    return (false, None);
                }
            }

            self.update_ray_hit_info(&ray, closest_t, &mut (*rec));

            return (true, Some(rec));
        }
    }

    pub fn closest_hit<'a>(
        &'a self,
        ray: &Ray,
        tmin: f32,
        tmax: f32,
        rec: &'a mut Intersection,
    ) -> (bool, Option<&mut Intersection>) {
        let oc = ray.origin - self.0.xyz();

        let a = dot(&ray.direction, &ray.direction);

        let half_b = dot(&oc, &ray.direction);

        let c = dot(&oc, &oc) - self.1 * self.1;

        let discriminant = half_b * half_b - a * c;

        if discriminant < 0.0 {
            return (false, None);
        }

        let mut closest_t = (-half_b - num::Float::sqrt(discriminant)) / a;

        if closest_t < tmin || tmax < closest_t {
            closest_t = (-half_b + num::Float::sqrt(discriminant)) / a;

            if closest_t < tmin || tmax < closest_t {
                return (false, None);
            }
        }

        // update hit intersect info
        rec.t = closest_t;

        rec.p = ray.origin + ray.direction * rec.t;

        let n = rec.p - self.0.xyz();

        rec.f = dot(&ray.direction, &n) < 0.0;

        rec.n = match rec.f {
            true => n.normalize(),
            false => -(n.normalize()),
        };

        let theta = acos(&-n.yy()).len() as f32;

        let phi = atan2(&-n.zz(), &n.xx()).len() as f32 + PI;

        rec.u = 0.5 * FRAC_1_PI * phi;

        rec.v = FRAC_1_PI * theta;

        return (true, Some(rec));
    }
}

impl Sphere {
    fn update_ray_hit_info(
        &self,
        ray: &Ray,
        t: f32,
        hit: &mut Intersection,
    ) -> bool {
        if t < 0.0 {
            return false;
        }

        (*hit).m = self.2;
        // p = ray.at(t)
        (*hit).p = ray.origin + ray.direction * t;

        // normal = P -c
        // https://raytracing.github.io/images/fig-1.05-sphere-normal.jpg
        let n = (1.0 / self.1) * ((*hit).p - self.0.xyz());
        hit.set_face_normal(ray, n);

        // ?
        let theta = acos(&-n.yy()).len() as f32;
        let phi = atan2(&-n.zz(), &n.xx()).len() as f32 + PI;
        (*hit).u = 0.5 * FRAC_1_PI * phi;
        (*hit).v = FRAC_1_PI * theta;

        true
    }

    // add code here
}

pub struct Lambertian<'a> {
    ray: &'a Ray,
    albedo: Vec3,
}

pub struct Metal<'a> {
    ray: &'a Ray,
    albedo: Vec3,
}

pub trait Scatterable {
    fn scatter(
        &mut self,
        rec: &Intersection,
    ) -> (Vec3, Ray);

    fn scatter_raw(
        &mut self,
        rec: *mut Intersection,
        attenuation: *mut Vec3,
        ray_scattered: *mut Ray,
    ) -> bool;
}

fn scatter_lambertian(
    ray: &Ray,
    rec: *mut Intersection,
    ray_scattered: *mut Ray,
) -> bool {
    if ray_scattered == std::ptr::null_mut() {
        return false;
    }
    unsafe {
        let scatter_direction = (*rec).p - random_unit_vector();

        let temp_ray = Ray::new(ray.origin, scatter_direction);

        (*ray_scattered).origin = temp_ray.origin;

        (*ray_scattered).direction = temp_ray.direction;

        true
    }
}
fn scatter_metal(
    ray: &Ray,
    rec: *mut Intersection,
    ray_scattered: *mut Ray,
) -> bool {
    if ray_scattered == std::ptr::null_mut() {
        return false;
    }
    unsafe {
        let reflected = reflect(unit_vertor(ray.direction), (*rec).n);

        let temp_ray = Ray::new((*rec).p, reflected);

        (*ray_scattered).origin = temp_ray.origin;

        (*ray_scattered).direction = temp_ray.direction;

        if dot(&(*ray_scattered).direction, &(*rec).n) > 0.0 {
            return true;
        }

        return false;
    }
}

pub fn default_background(ray: &Ray) -> Rgb<u8> {
    let unit_direction = ray.direction.normalize();

    let t = 0.5 * (unit_direction.y + 1.0);

    let start_color_v3 = glm::vec3(0.6, 0.6, 0.75);

    let end_color_v3 = glm::vec3(0.08, 0.05, 0.02);

    let background_color_v3 = (1.0 - t) * start_color_v3 + t * end_color_v3;

    let background_color = vec3_to_rgb8(255.0 * background_color_v3);

    background_color
}

pub fn gradient_background(ray: &Ray) -> Rgb<u8> {
    Rgb([ray.direction.y as u8, ray.direction.x as u8, 50])
}
