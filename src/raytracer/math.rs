use image::Rgb;
use nalgebra_glm::{dot, vec3, Vec3};

pub fn coord_to_color(
    u_or_v: u32,
    w_or_h: f32,
) -> f32 {
    (u_or_v as f32 / w_or_h as f32)
}

pub fn arry_to_vec3(color: [f32; 3]) -> Vec3 {
    vec3(color[0] as f32, color[1] as f32, color[2] as f32)
}

pub fn vec3_to_rgb8(v: Vec3) -> Rgb<u8> {
    Rgb([v.x as u8, v.y as u8, v.z as u8])
}

pub fn rgb8_to_vec3(color: Rgb<u8>) -> Vec3 {
    vec3(color[0] as f32, color[1] as f32, color[2] as f32)
}

pub fn to_rgb8(x: f32) -> u8 {
    (x * 255.0) as u8
}
pub fn plus_rgb8(
    color1: Rgb<u8>,
    color2: Rgb<u8>,
) -> Rgb<u8> {
    Rgb([
        color1[0] + color2[0],
        color1[1] + color2[1],
        color1[2] + color2[2],
    ])
}
pub fn minus_rgb8(
    color1: Rgb<u8>,
    color2: Rgb<u8>,
) -> Rgb<u8> {
    Rgb([
        color1[0] - color2[0],
        color1[1] - color2[1],
        color1[2] - color2[2],
    ])
}
pub fn mult_rgb8(
    color1: Rgb<u8>,
    multiplier: u8,
) -> Rgb<u8> {
    Rgb([
        color1[0] * multiplier,
        color1[1] * multiplier,
        color1[2] * multiplier,
    ])
}
pub fn div_rgb8(
    color1: Rgb<u8>,
    dinominator: u8,
) -> Rgb<u8> {
    Rgb([
        color1[0] / dinominator,
        color1[1] / dinominator,
        color1[2] / dinominator,
    ])
}
pub fn rgb8_from_vec3(color: [f32; 3]) -> Rgb<u8> {
    let r = to_rgb8(color[0]);
    let g = to_rgb8(color[1]);
    let b = to_rgb8(color[2]);

    Rgb([r, g, b])
}
pub fn adjust_gamma_color(
    color: Vec3,
    n_samples: u32,
) -> Vec3 {
    let color_gamma1 = color / n_samples as f32;

    color_gamma1
}

pub fn adjust_gamma2_color(
    color: Vec3,
    n_samples: u32,
) -> Vec3 {
    let color_gamma1 = adjust_gamma_color(color, n_samples);
    let color_gamma2 = vec3(
        num::Float::sqrt(color_gamma1.x),
        num::Float::sqrt(color_gamma1.y),
        num::Float::sqrt(color_gamma1.z),
    );
    color_gamma2
}
pub fn clamp<T: std::cmp::PartialOrd>(
    x: T,
    min: T,
    max: T,
) -> T {
    if x < min {
        return min;
    };
    if x > max {
        return max;
    };
    return x;
}
pub fn random_f32() -> f32 {
    // Returns a random real in [0,1).
    return rand::random::<f32>() / (std::f32::MAX + 1.0);
}
pub fn random_double_rng(
    min: f32,
    max: f32,
) -> f32 {
    // Returns a random real in [0,1).
    return min + (max - min) * random_f32();
}
pub fn vec3_random(
    min: f32,
    max: f32,
) -> Vec3 {
    vec3(
        random_double_rng(min, max),
        random_double_rng(min, max),
        random_double_rng(min, max),
    )
}
pub fn random_in_unit_sphere() -> Vec3 {
    loop {
        if false {
            break;
        }
        let p = vec3_random(-1.0, 1.0);
        return p;
    }
    return glm::vec3(0.0, 0.0, 0.0);
}

pub fn random_in_hemisphere(normal: Vec3) -> Vec3 {
    let in_unit_sphere = random_in_unit_sphere();
    if dot(&in_unit_sphere, &normal) > 0.0 {
        return in_unit_sphere;
    } // In the same hemisphere as the normal
    return -in_unit_sphere;
}

pub fn unit_vertor(v: Vec3) -> Vec3 {
    v / v.len() as f32
}
pub fn random_unit_vector() -> Vec3 {
    return unit_vertor(random_in_unit_sphere());
}

pub fn reflect(
    v: Vec3,
    n: Vec3,
) -> Vec3 {
    v - 2.0 * dot(&v, &n) * n
}
