use crate::raytracer::*;

pub struct Color {
    data: Vec3,
}

impl Color {
    pub fn mul_vector(
        &mut self,
        v: Vec3,
    ) {
        self.data.x *= v.x;
        self.data.y *= v.y;
        self.data.z *= v.z;
    }
    pub fn mul_f32(
        &mut self,
        m: f32,
    ) {
        self.data.x *= m;
        self.data.y *= m;
        self.data.z *= m;
    }
}

impl Color {
    pub fn new() -> Self {
        Self {
            data: vec3(0.0, 0.0, 0.0),
        }
    }
}
