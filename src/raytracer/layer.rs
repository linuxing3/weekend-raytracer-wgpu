use crate::fly_camera::FlyCameraController;

use super::{texture::*, RenderParams};
use image::{DynamicImage, Rgb};
use imgui::TextureId;

pub struct Layer<'a> {
    texture_id: imgui::TextureId,
    pub size: [f32; 2],
    pub title: &'a str,
    pub file_path: &'a str,
    imgbuf: *mut XImageBuffer,
    params: RenderParams,
}

impl<'a> Layer<'a> {
    pub fn new(
        size: [f32; 2],
        title: &'a str,
        file_path: &'a str,
        params: RenderParams,
    ) -> Self {
        let image_buffer =
            Texture::new_from_image_buffer(size[0] as u32, size[1] as u32, file_path).unwrap();

        let imgbuf_boxed = Box::new(image_buffer);
        let imgbuf = unsafe { Box::into_raw(imgbuf_boxed) };

        let texture_id = TextureId::new(0);

        Self {
            texture_id,
            size,
            title,
            file_path,
            imgbuf,
            params,
        }
    }

    pub fn register_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut imgui_wgpu::Renderer,
    ) -> Option<TextureId> {
        let imgbuf_boxed = unsafe { Box::from_raw(self.imgbuf) };
        let dimensions = imgbuf_boxed.dimensions();

        let img = DynamicImage::from(*imgbuf_boxed);

        let bytes: &[u8] = &img.to_rgba8();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
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

    pub fn img_buf(&mut self) -> XImageBuffer {
        let imgbuf_boxed = unsafe { Box::from_raw(self.imgbuf) };
        let mut imgbuf = *imgbuf_boxed;
        imgbuf
    }

    pub fn render(
        &mut self,
        ui: &mut imgui::Ui,
    ) {
        let window = ui.window(self.title);

        let mut new_imgui_region_size = None;

        window
            .size(self.size, imgui::Condition::FirstUseEver)
            .build(|| {
                new_imgui_region_size = Some(ui.content_region_avail());

                ui.text("Moon");

                imgui::Image::new(self.texture_id, new_imgui_region_size.unwrap()).build(ui);
            });
    }
    pub fn resize(
        &mut self,
        ui: &mut imgui::Ui,
        size: [f32; 2],
        params: &RenderParams,
        camera: &FlyCameraController,
    ) {
        if self.size != size {
            self.size = size;
        }

        self.render(ui);
    }
}
