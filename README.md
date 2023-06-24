# README

## Ray Tracing

in `layer.rs` file, we trace ray!!!

### loop pixels

```rust

    pub fn set_data(&mut self) {
        let [width, height] = self.vp_size;
        unsafe {
            // A redundant loop to demonstrate reading image data
            for y in 0..height as u32 {
                for x in 0..width as u32 {
                    let pixel = (*self.imgbuf).get_pixel_mut(x, y);

                    let u = coord_to_color(x, width);

                    let v = coord_to_color(y, height);

                    *pixel = self.per_pixel(u, v);
                }
            }
        }
    }

```

### set color per pixel, with multisampling

#### formula quadradic

$
t = \sqrt ( b ^ 2 - 4 * a * c  )
$

```rust

    pub fn per_pixel(
        &mut self,
        x: f32,
        y: f32,
    ) -> Rgb<u8> {
        let (u, v) = ((x + 1.0) / 2.0, (y + 1.0) / 2.0);

        // pixel color for multisampling
        for sphere in &self.world {
            let mut pixel_color = Rgb([0_u8, 0_u8, 0_u8]);
            // multisampling
            for _s in 0..100 {
                // FIXME: include random (-1.0 - 1.0)
                let su = u + 0.010;
                let sv = v + 0.010;
                let mut ray = self.camera.get_ray(su, sv);
                let root = Self::trace_ray(&mut ray, *sphere, 0.0, num::Float::max_value());

                let hit = Self::get_ray_hit(&mut ray, *sphere, root);

                let nn = hit.n.normalize();

                let ray_color = rgb8_from_vec3([nn.x, nn.y, nn.z]);

                pixel_color = plus_rgb8(pixel_color, ray_color);

                let background_color = rgb8_from_vec3([x, y, 50.0]);
                if hit.t >= 0.0 {
                    return pixel_color;
                } else {
                    return background_color;
                }
            }
        }

        // when world is empty
        rgb8_from_vec3([0.0, 0.0, 0.0])
    }

```

### trace ray

```rust

    pub fn trace_ray(
        ray: &Ray,
        sphere: Sphere,
        tmin: f32,
        tmax: f32,
    ) -> f32 {
        let oc = ray.origin - sphere.center.xyz();

        let a = dot(&ray.direction, &ray.direction);

        let half_b = dot(&oc, &ray.direction);

        let c = dot(&oc, &oc) - sphere.radius * sphere.radius;

        let discriminant = half_b * half_b - a * c;

        if discriminant > 0.0 {
            // closet T
            let mut root = (-half_b - num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                return root;
            }

            // farest T
            root = (-half_b + num::Float::sqrt(discriminant)) / a;

            if root < tmax && root > tmin {
                return root;
            }
        }

        return -1.0;
    }

```

## Asset credits

assets/moon.jpeg

- NASA's Scientific Visualization Studio
- https://svs.gsfc.nasa.gov/4720

assets/earthmap.jpeg

- https://raytracing.github.io/images/earthmap.jpg

## Acknowledge

### Light

### diffuse

formula

$
\sin (\theta) + \cos ()
$

### reflect

### refract

### camera

#### fov (field of view)

### lens (aperture disk)

### focus

### view plane

###
