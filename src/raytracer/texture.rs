use imgui::TextureId;
use nalgebra_glm::{dot, Vec3};
use thiserror::Error;

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, RgbaImage};

use super::{ray_intersect_sphere, Ray, Sphere};

type XImageBuffer = ImageBuffer<Rgb<u8>, Vec<u8>>;

pub struct Texture {
    dimensions: (u32, u32),
    data: Vec<[f32; 3]>,
}

pub struct WgpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

#[derive(Default)]

pub struct CustomImguiTextures<'a> {
    texture_id: Option<TextureId>,
    width: u32,
    height: u32,
    path: &'a str,
}

impl<'a> CustomImguiTextures<'a> {
    pub fn new(
        width: u32,
        height: u32,
        path: &'a str,
    ) -> Self {
        CustomImguiTextures {
            texture_id: None,
            width,
            height,
            path,
        }
    }

    pub fn register_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> Option<TextureId> {
        let imgbuf = Texture::new_from_image_buffer(self.width, self.height, self.path).unwrap();

        let dimensions = imgbuf.dimensions();

        let img = DynamicImage::from(imgbuf);

        let bytes: &[u8] = &img.to_rgba8();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let imgui_texture: _ =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &bytes, size);

        self.texture_id = Some(renderer.textures.insert(imgui_texture));

        self.texture_id
    }

    pub fn update_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
        width: u32,
        height: u32,
        texture_id: TextureId,
        path: &str,
    ) -> Option<TextureId> {
        let mut imgbuf = Texture::new_from_image_buffer(width, height, path).unwrap();

        let dimensions = imgbuf.dimensions();

        let mut raw_data: Vec<u8> = Vec::new();

        // Iterate over the coordinates and pixels of the image
        for (x, y, pixel) in imgbuf.enumerate_pixels_mut() {
            let image::Rgb(data) = *pixel;

            for i in 0..3 {
                raw_data.push(data[i]);

                raw_data.push(1);
            }
        }

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let imgui_texture: _ =
            WgpuTexture::new_imgui_texture(&device, &queue, &renderer, &raw_data, size);

        match renderer.textures.replace(texture_id, imgui_texture) {
            Some(t) => Some(texture_id),
            _ => None,
        }
    }
}

impl Texture {
    pub fn per_pixel_with_raytracing(
        x: f32,
        y: f32,
    ) -> Rgb<u8> {
        let origin = glm::vec3(1.5, 1.5, -4.0);

        let direction = glm::vec3(x, y, -1.0);

        let ray = Ray { origin, direction };

        let radius = 0.5_f32;

        let a = dot(&ray.direction, &ray.direction);

        let b = 2.0 * dot(&ray.origin, &ray.direction);

        let c = dot(&ray.origin, &ray.origin) - radius * radius;

        let discriminant = b * b - 4.0 * a * c;

        // println!(" Coords: [{}, {}] ", x, y);
        // println!(" Color:  [{}, {}, {} -> {}] ", a, b, c, discriminant);

        match discriminant >= 0.0 {
            true => Rgb([255.0 as u8, 18.0 as u8, 18.0 as u8]),
            false => Rgb([10.0 as u8, 25.0 as u8, 255.0 as u8]),
        }
    }

    pub fn set_pixel_with_uniform_color(
        imgbuf: &mut XImageBuffer,
        color: Rgb<u8>,
    ) {
        // Iterate over the coordinates and pixels of the image
        for (x, y, per_pixel) in imgbuf.enumerate_pixels_mut() {
            *per_pixel = color;
        }
    }

    pub fn set_pixels_with_raytracing(
        width: u32,
        height: u32,
        imgbuf: &mut XImageBuffer,
    ) {
        // A redundant loop to demonstrate reading image data
        for y in 0..height {
            for x in 0..width {
                let pixel = imgbuf.get_pixel_mut(x, y);

                *pixel = Texture::per_pixel_with_raytracing(
                    x as f32 / width as f32,
                    y as f32 / height as f32,
                );
            }
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

        image::Rgb([i as u8, i as u8, i as u8])
    }

    pub fn new_from_image_buffer(
        width: u32,
        height: u32,
        _path: &str,
    ) -> Result<XImageBuffer, TextureError> {
        // Create a new ImgBuf with width: imgx and height: imgy
        let mut imgbuf = ImageBuffer::new(width, height);

        Texture::set_pixels_with_raytracing(width, height, &mut imgbuf);

        // Save the image as “fractal.png”, the format is deduced from the path
        // imgbuf.save(path).unwrap();

        Ok(imgbuf)
    }

    pub fn new_from_image(path: &str) -> Result<Self, TextureError> {
        use std::fs::*;
        use std::io::BufReader;

        let file = File::open(path)?;

        let pixels: RgbaImage =
            image::load(BufReader::new(file), image::ImageFormat::Jpeg)?.into_rgba8();

        let inv_255 = 1_f32 / 255_f32;

        let dimensions = pixels.dimensions();

        println!("w{} h{}", dimensions.0, dimensions.1);

        let data: Vec<_> = pixels
            .pixels()
            .map(|p| -> [f32; 3] {
                [
                    inv_255 * (p[0] as f32),
                    inv_255 * (p[1] as f32),
                    inv_255 * (p[2] as f32),
                ]
            })
            .collect();

        println!("SliceF32Array Size: {}", data.as_slice().len());

        Ok(Self { dimensions, data })
    }

    pub fn new_from_color(color: glm::Vec3) -> Self {
        let data = vec![[color.x, color.y, color.z]];

        let dimensions = (1_u32, 1_u32);

        Self { dimensions, data }
    }

    pub fn as_slice(&self) -> &[[f32; 3]] {
        self.data.as_slice()
    }

    pub fn as_vec_u8(&self) -> Vec<u8> {
        self.data
            .as_slice()
            .into_iter()
            .map(|s| -> [u8; 3] {
                [
                    (s[0] * 255_f32) as u8,
                    (s[1] * 255_f32) as u8,
                    (s[2] * 255_f32) as u8,
                ]
            })
            .flatten()
            .collect()
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }
}

impl WgpuTexture {
    pub fn new_imgui_texture<'a>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &imgui_wgpu::Renderer,
        raw_data: &[u8],
        size: wgpu::Extent3d,
    ) -> imgui_wgpu::Texture {
        let texture_config = imgui_wgpu::TextureConfig {
            size,
            label: Some("raw texture"),
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        };

        let texture = imgui_wgpu::Texture::new(device, renderer, texture_config);

        texture.write(&queue, &raw_data, size.width, size.height);

        texture
    }

    pub fn new_wgpu_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: Option<&str>,
    ) -> Result<Self, TextureError> {
        let img = image::load_from_memory(bytes)?;

        let dimensions = img.dimensions();

        let rgba = img.to_rgba8();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}

#[derive(Error, Debug)]

pub enum TextureError {
    #[error(transparent)]
    FileIoError(#[from] std::io::Error),
    #[error(transparent)]
    ImageLoadError(#[from] image::ImageError),
}
