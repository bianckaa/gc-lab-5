//! EXTRA "Cristalis": mundo de hielo fracturado con auroras polares.

use super::utils::*;
use super::{opaque, FragmentIn};
use nalgebra_glm as glm;

const PLATES: [Stop; 5] = [
    (0.00, [0.30, 0.70, 0.88]), // placas recongeladas color turquesa
    (0.18, [0.64, 0.86, 0.98]),
    (0.50, [0.82, 0.93, 1.00]),
    (0.80, [0.95, 0.98, 1.00]),
    (1.00, [0.58, 0.80, 0.98]),
];

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;

    // CAPA 1: placas de hielo por voronoi, cada una con su tono y escarcha.
    let plates = voronoi(&warp(&(n * 3.4), 0.45, 2));
    let frost = fbm(&(n * 16.0), 3);
    let streaks = fbm(&(n.component_mul(&v3(4.0, 22.0, 4.0)) + v3(plates.id * 9.0, 0.0, 0.0)), 2);
    let mut color = ramp(&PLATES, plates.id) * (0.93 + 0.22 * frost + 0.12 * streaks);

    // CAPA 2: grietas azul profundo (bordes de placas + fisuras finas + crestas de presión).
    let main_crack = smoothstep(0.07, 0.0, plates.edge());
    let fine = voronoi(&(n * 11.0 + v3(3.0, 7.0, 1.0)));
    let fine_crack = smoothstep(0.035, 0.0, fine.edge()) * smoothstep(0.2, 0.5, fbm(&(n * 3.0), 3) + 0.3);
    let pressure = smoothstep(0.82, 0.95, ridged(&(n * 6.0 + v3(1.0, 2.0, 3.0)), 3));
    let crack_glow = smoothstep(0.22, 0.0, plates.edge());
    color = mix3(&color, &rgb(0x48c8f0), crack_glow * 0.35); // luz filtrada por el hielo
    color = mix3(&color, &rgb(0x0a2a6a), main_crack * 0.92);
    color = mix3(&color, &rgb(0x2a5fa8), fine_crack * 0.6);
    color = mix3(&color, &rgb(0xf4fbff), pressure * 0.5);

    // Iluminación base con dispersión subsuperficial azul en el terminador.
    let shadow = f.external_shadow();
    let ndl = glm::dot(&f.normal, &f.light_dir);
    let diffuse = smoothstep(-0.2, 1.0, ndl) * shadow;
    let subsurface = smoothstep(-0.35, 0.0, ndl) * smoothstep(0.45, 0.0, ndl) * shadow;
    let mut lit = color * (0.025 + diffuse) + rgb(0x1d6fd6) * subsurface * 0.12;

    // CAPA 3: brillo cristalino (especular alto + destellos de cristales).
    let spec = specular(&f.normal, &f.light_dir, &f.view_dir, 140.0) * shadow;
    let broad = specular(&f.normal, &f.light_dir, &f.view_dir, 18.0) * shadow;
    let sparkle_cell = hash_point(&n, 260.0);
    let twinkle = 0.5 + 0.5 * (t * 3.0 + sparkle_cell * 60.0).sin();
    let sparkle = smoothstep(0.985, 1.0, sparkle_cell) * twinkle * diffuse;
    lit += rgb(0xffffff) * spec * 1.2 * (1.0 - main_crack) + rgb(0xbfe6ff) * broad * 0.2;
    lit += rgb(0xe8f6ff) * sparkle * 1.5;

    // CAPA 4: auroras polares verdes y violetas, animadas (cortinas onduladas).
    let lat = n.y.abs();
    let lon = n.z.atan2(n.x);
    let hemisphere = n.y.signum();
    let wobble = 0.035 * (lon * 5.0 + t * 0.7 * hemisphere).sin() + 0.02 * (lon * 11.0 - t * 1.1).sin();
    let oval = (-((lat - 0.82 - wobble) / 0.07).powi(2)).exp();
    let curtains = smoothstep(-0.05, 0.3, fbm(&v3(lon * 4.0, t * 0.35, hemisphere * 3.0), 3));
    let shimmer = 0.75 + 0.25 * (lon * 40.0 + t * 4.0).sin();
    let aurora_color = mix3(&rgb(0x3dff8e), &rgb(0xa45bff), smoothstep(0.78, 0.9, lat + wobble));
    let night = night_factor(&f.normal, &f.light_dir);
    lit += aurora_color * oval * curtains * shimmer * (1.1 + 2.2 * night);

    // Borde atmosférico tenue color aguamarina.
    lit += rgb(0x9fe3ff) * fresnel(&f.normal, &f.view_dir, 3.0) * (0.06 + 0.5 * diffuse);

    opaque(lit)
}
