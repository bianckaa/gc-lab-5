//! ESTRELLA "Helios Ardens": el sol del sistema.
//! Es emisivo (no recibe sombra) y además es la fuente de luz de la escena.

use super::halo::ray_closest;
use super::utils::*;
use super::{opaque, FragmentIn};
use nalgebra_glm as glm;

const LIMB: [Stop; 4] = [
    (0.00, [0.90, 0.25, 0.02]),
    (0.30, [1.00, 0.52, 0.06]),
    (0.70, [1.00, 0.74, 0.22]),
    (1.00, [1.00, 0.88, 0.48]),
];

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;
    // mu = coseno entre la normal y la vista: 1 al centro del disco, 0 en el limbo.
    let mu = glm::dot(&f.normal, &f.view_dir).max(0.0);

    // CAPA 1: gradiente núcleo -> borde con oscurecimiento del limbo.
    let mut color = ramp(&LIMB, mu.powf(0.6)) * (0.62 + 0.38 * mu.powf(0.45)) * 1.15;

    // CAPA 2: granulación convectiva (voronoi animado + fbm de plasma).
    let flow = warp(&(n * 3.0 + v3(0.0, t * 0.05, 0.0)), 0.35, 2);
    let cell = voronoi(&(flow * 13.0 + v3(t * 0.08, -t * 0.05, t * 0.06)));
    let granule = smoothstep(0.8, 0.0, cell.f1); // centro brillante de la celda
    let lanes = smoothstep(0.0, 0.3, cell.edge()); // carriles intergranulares oscuros
    let plasma = fbm(&(n * 6.0 + v3(t * 0.12, t * 0.07, -t * 0.1)), 4);
    color *= (0.84 + 0.2 * granule) * (0.9 + 0.1 * lanes);
    color += rgb(0xfff0b0) * smoothstep(0.05, 0.35, plasma) * 0.35 * mu;

    // CAPA 3: manchas solares en latitudes medias (umbra oscura + penumbra rojiza).
    let lat = n.y.abs();
    let belt = smoothstep(0.05, 0.15, lat) * smoothstep(0.62, 0.40, lat);
    let spots = fbm(&(rotate_y(&n, t * 0.01) * 2.4 + v3(7.0, 1.0, 3.0)), 4) * belt;
    let penumbra = smoothstep(0.17, 0.23, spots);
    let umbra = smoothstep(0.245, 0.29, spots);
    let filaments = 0.75 + 0.25 * noise(&(n * 40.0));
    color = mix3(&color, &(rgb(0x8a2a08) * filaments), penumbra * 0.85);
    color = mix3(&color, &rgb(0x1c0702), umbra);

    // CAPA 4: corona y llamaradas en el limbo (fresnel emisivo, ruido animado y pulso).
    let rim = fresnel(&f.normal, &f.view_dir, 2.5);
    let flare = fbm(&(n * 5.0 + v3(0.0, t * 0.35, t * 0.2)), 3);
    let pulse = 0.85 + 0.15 * (t * 2.2 + n.y * 4.0).sin();
    color += rgb(0xff8a1f) * rim * (0.7 + 2.2 * smoothstep(-0.05, 0.3, flare)) * pulse;

    opaque(color)
}

/// Corona exterior: cáscara aditiva con serpentinas y protuberancias animadas.
pub fn corona(f: &FragmentIn, shell: f32) -> Option<Vec4> {
    let (d, q) = ray_closest(f);
    let x = saturate((d - 1.0) / (shell - 1.0));
    if x >= 1.0 {
        return None;
    }
    let t = f.time;
    let dir = glm::normalize(&(q - f.ctx.center));
    // Serpentinas: ruido angular que cambia con lentitud.
    let streamers = fbm(&(dir * 3.5 + v3(0.0, 0.0, t * 0.08)), 4);
    let reach = 3.2 - 2.0 * smoothstep(-0.2, 0.35, streamers);
    let glow = (-x * reach * 2.0).exp() * (1.0 - x);
    // Protuberancias: filamentos brillantes muy cerca del limbo.
    let loops = ridged(&(dir * 7.0 + v3(t * 0.15, 0.0, -t * 0.1)), 3);
    let prominence = smoothstep(0.72, 0.92, loops) * smoothstep(0.22, 0.0, x);
    let pulse = 0.9 + 0.1 * (t * 1.7).sin();
    let color = mix3(&rgb(0xff4a08), &rgb(0xffd27a), (1.0 - x).powf(3.0));
    let c = color * (glow * 1.4 + prominence * 1.6) * pulse;
    Some(Vec4::new(c.x, c.y, c.z, 1.0))
}
