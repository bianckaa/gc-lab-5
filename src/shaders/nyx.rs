//! EXTRA "Nyx Lumina": planeta alienígena bioluminiscente.

use super::utils::*;
use super::{opaque, FragmentIn};

const BASE: [Stop; 4] = [
    (0.00, [0.03, 0.01, 0.07]),
    (0.35, [0.12, 0.03, 0.20]),
    (0.70, [0.27, 0.07, 0.36]),
    (1.00, [0.42, 0.14, 0.48]),
];

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;

    // CAPA 1: base violeta oscuro con domain warping (formas orgánicas que fluyen).
    let q = warp(&(n * 1.8 + v3(0.0, t * 0.01, 0.0)), 0.9, 3);
    let organic = fbm(&warp(&(q * 1.5), 0.6, 2), 5);
    let base = ramp(&BASE, smoothstep(-0.3, 0.3, organic)) * 1.25;

    let shadow = f.external_shadow();
    let diffuse = soft_diffuse(&f.normal, &f.light_dir, 0.25) * shadow;
    let night = night_factor(&f.normal, &f.light_dir);
    let mut color = base * (0.05 + 0.95 * diffuse);
    // La bioluminiscencia resalta más en el lado nocturno.
    let glow_boost = 0.75 + 0.9 * night;

    // CAPA 2: venas bioluminiscentes turquesa con pulsos que viajan por ellas.
    let vein_field = noise(&(q * 2.3));
    let veins = (1.0 - vein_field.abs()).powf(36.0);
    let branch_mask = smoothstep(-0.05, 0.2, fbm(&(n * 2.5 + v3(7.0, 3.0, 1.0)), 3));
    let branches = (1.0 - noise(&(q * 6.0 + v3(3.0, 1.0, 2.0))).abs()).powf(60.0) * 0.5 * branch_mask;
    let travel = 0.5 + 0.5 * (t * 2.4 - organic * 18.0).sin();
    let pulse = 0.35 + 0.65 * travel * travel;
    let vein_light = (veins + branches) * pulse;
    color += rgb(0x19ffd8) * vein_light * 1.3 * glow_boost;
    color += rgb(0x0a8f8a) * (1.0 - vein_field.abs()).powf(8.0) * 0.18 * glow_boost; // resplandor difuso

    // CAPA 3: manchas de esporas magenta que "respiran" (voronoi).
    let spores = voronoi(&(warp(&(n * 5.0), 0.3, 2)));
    if spores.id > 0.5 {
        let size = 0.32 + 0.18 * fract(spores.id * 11.0);
        let d = spores.f1 / size;
        let breathe = 0.6 + 0.4 * (t * 1.3 + spores.id * 30.0).sin();
        let halo = smoothstep(1.0, 0.15, d);
        let speckle = smoothstep(0.55, 0.85, value_noise(&(n * 90.0))) * halo;
        let core = smoothstep(0.3, 0.0, d);
        color = color * (1.0 - halo * 0.4) + rgb(0xff2fa6) * halo * halo * 0.9 * breathe * glow_boost;
        color += rgb(0xff7ad0) * speckle * 0.8 * breathe * glow_boost;
        color += rgb(0xffd0f0) * core * breathe * glow_boost;
    }

    // CAPA 4: halo atmosférico cian con fresnel (más un brillo especular húmedo).
    let rim = fresnel(&f.normal, &f.view_dir, 2.2);
    color += rgb(0x29e6ff) * rim * (0.35 + 0.65 * diffuse) * 0.9;
    color += rgb(0xc9a6ff) * specular(&f.normal, &f.light_dir, &f.view_dir, 40.0) * 0.25 * shadow;

    opaque(color)
}
