//! PLANETA ROCOSO "Terra Viridis" (tipo Tierra): el cuerpo más complejo.

use super::utils::*;
use super::{opaque, FragmentIn};
use nalgebra_glm as glm;

/// Nivel del mar sobre el fbm de elevación (~35 % de tierra firme).
const SEA_LEVEL: f32 = 0.045;

const OCEAN: [Stop; 4] = [
    (0.00, [0.10, 0.62, 0.66]), // aguas someras turquesa
    (0.12, [0.05, 0.36, 0.56]),
    (0.45, [0.02, 0.15, 0.38]),
    (1.00, [0.01, 0.05, 0.18]), // abismo azul marino
];

/// Elevación de la superficie (positiva = tierra firme).
fn elevation(n: &Vec3) -> f32 {
    let q = warp(&(n * 1.35), 0.55, 3);
    fbm(&(q * 1.6), 6) - SEA_LEVEL
}

/// Cobertura de nubes en un punto del dominio de nubes.
fn clouds(nc: &Vec3, t: f32) -> f32 {
    let q = warp(&(nc * 2.2 + v3(0.0, 0.0, t * 0.01)), 0.6, 2);
    let c = fbm(&(q * 2.4), 5);
    // Bandas de convergencia (más nubes en el ecuador y latitudes medias).
    let belts = 0.06 * (nc.y * 9.0).cos();
    let wisps = 0.08 * fbm(&(nc * 14.0), 2);
    smoothstep(0.06, 0.32, c + belts + wisps)
}

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;
    let l_obj = f.light_obj();
    let lat = n.y.abs();
    let e = elevation(&n);
    let land = smoothstep(-0.004, 0.004, e);

    // CAPA 1: océanos con gradiente de profundidad y brillo especular.
    let depth = saturate(-e * 4.5);
    let ocean = ramp(&OCEAN, depth);

    // CAPA 2: continentes y biomas por altura, latitud y humedad.
    let altitude = saturate(e / 0.22);
    let moisture = fbm(&(n * 2.8 + v3(11.0, 3.0, -4.0)), 4) + 0.18 * (1.0 - lat * 1.6);
    let detail = fbm(&(n * 22.0), 3);
    let forest = rgb(0x1f5a1c);
    let grass = rgb(0x5f8a2e);
    let desert = rgb(0xd2a95e);
    let ochre = rgb(0x9b6b35);
    let tundra = rgb(0x6c6f55);
    let rock = rgb(0x6b5a4a);
    let mut ground = mix3(&grass, &forest, smoothstep(0.0, 0.18, moisture));
    // Desiertos en las franjas subtropicales secas.
    let dry = smoothstep(0.02, -0.12, moisture) * smoothstep(0.62, 0.28, lat);
    ground = mix3(&ground, &mix3(&ochre, &desert, smoothstep(-0.1, -0.2, moisture)), dry);
    ground = mix3(&ground, &tundra, smoothstep(0.52, 0.68, lat));
    ground = mix3(&ground, &rock, smoothstep(0.35, 0.75, altitude + detail * 0.4));
    // Franja de playa junto a la costa.
    ground = mix3(&rgb(0xc9b98a), &ground, smoothstep(0.0, 0.012, e));
    ground *= 0.88 + 0.35 * detail;

    let mut surface = mix3(&ocean, &ground, land);

    // CAPA 3: casquetes polares y picos nevados (latitud + altura con borde ruidoso).
    let edge_noise = fbm(&(n * 9.0 + v3(2.0, 0.0, 5.0)), 3) * 0.10;
    let cap = smoothstep(0.90, 0.93, lat + edge_noise + land * 0.04);
    let peaks = smoothstep(0.86, 0.94, altitude + edge_noise * 1.5 + lat * 0.25) * land;
    let snow = cap.max(peaks);
    surface = mix3(&surface, &rgb(0xeef3f8), snow);

    // CAPA 4: nubes que derivan con el tiempo y su sombra sobre la superficie.
    let nc = rotate_y(&n, t * 0.035);
    let cloud = clouds(&nc, t);
    let cloud_shadow = clouds(&glm::normalize(&(nc - rotate_y(&l_obj, t * 0.035) * 0.025)), t);
    surface *= 1.0 - 0.5 * cloud_shadow * (1.0 - cloud);

    // Iluminación: difuso con terminador suave + eclipses/sombras externas.
    let shadow = f.external_shadow();
    let diffuse = soft_diffuse(&f.normal, &f.light_dir, 0.12) * shadow;
    let water = (1.0 - land) * (1.0 - snow) * (1.0 - cloud);
    let glint = specular(&f.normal, &f.light_dir, &f.view_dir, 90.0)
        * (0.75 + 0.25 * noise(&(n * 60.0 + v3(t * 0.3, 0.0, 0.0))));
    let mut color = surface * (0.025 + diffuse) + rgb(0xfff4dc) * glint * water * 0.9 * shadow;

    // Nubes iluminadas encima de todo.
    let cloud_color = mix3(&rgb(0x7a8596), &rgb(0xfbfcff), diffuse);
    color = mix3(&color, &(cloud_color * (0.04 + diffuse)), cloud * 0.95);

    // Extra: luces de ciudades en el lado nocturno (costas templadas, sin hielo ni nubes).
    let night = night_factor(&f.normal, &f.light_dir);
    let clusters = smoothstep(0.05, 0.22, fbm(&(n * 7.0 + v3(-5.0, 2.0, 8.0)), 3));
    let lights = smoothstep(0.62, 0.9, value_noise(&(n * 140.0)));
    let habitable = land * (1.0 - snow) * (1.0 - dry) * smoothstep(0.7, 0.4, lat) * smoothstep(0.3, 0.0, altitude - 0.1);
    color += rgb(0xffb550) * lights * clusters * habitable * night * (1.0 - cloud * 0.85) * 1.4;

    // Extra: halo atmosférico azul en el disco (fresnel), más intenso del lado iluminado.
    let rim = fresnel(&f.normal, &f.view_dir, 3.0);
    color += rgb(0x4f9dff) * rim * (0.08 + 0.9 * soft_diffuse(&f.normal, &f.light_dir, 0.35));

    opaque(color)
}
