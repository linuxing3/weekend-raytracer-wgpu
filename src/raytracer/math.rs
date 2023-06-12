use image::Rgb;
pub fn coord_to_color(
    u_or_v: u32,
    w_or_h: f32,
) -> f32 {
    (u_or_v as f32 / w_or_h as f32) * 2.0 - 1.0
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
pub fn write_color(
    color: Rgb<u8>,
    n_samples: u32,
) -> Rgb<u8> {
    let mut r = color[0] as f32;
    let mut g = color[1] as f32;
    let mut b = color[2] as f32;
    let scale: f32 = (1 / n_samples) as f32;
    r *= scale;
    g *= scale;
    b *= scale;

    let rr = (256.0 * clamp(r, 0.0, 0.999)) as u8;
    let gg = (256.0 * clamp(g, 0.0, 0.999)) as u8;
    let bb = (256.0 * clamp(b, 0.0, 0.999)) as u8;

    Rgb([rr, gg, bb])
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