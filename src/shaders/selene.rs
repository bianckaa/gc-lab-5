//! LUNA "Selene": satélite de Terra Viridis (modelo separado: icosfera propia).

use super::utils::*;
use super::{opaque, FragmentIn};
use nalgebra_glm as glm;

/// Perfil de altura de un cráter en función de la distancia normalizada d:
/// cuenco hundido (d < 1) y borde elevado alrededor de d = 1.
#[inline]
fn crater_height(d: f32) -> f32 {
    let bowl = if d < 1.0 { 0.55 * (d * d - 1.0) } else { 0.0 };
    let rim = 0.32 * (-((d - 1.0) / 0.16).powi(2)).exp();
    bowl + rim
}

struct Craters {
    /// Oscurecimiento del fondo del cráter (0..1).
    floor: f32,
    /// Brillo del borde (0..1).
    rim: f32,
    /// Halo de eyecciones de cráteres jóvenes (0..1).
    ejecta: f32,
    /// Perturbación de la normal en espacio de objeto.
    bump: Vec3,
}

/// Una escala de cráteres basada en voronoi.
fn crater_layer(n: &Vec3, scale: f32, offset: Vec3, existence: f32, strength: f32) -> Craters {
    let p = n * scale + offset;
    let cell = voronoi(&p);
    let mut out = Craters { floor: 0.0, rim: 0.0, ejecta: 0.0, bump: Vec3::zeros() };
    if cell.id < existence {
        return out;
    }
    let size = 0.22 + 0.22 * fract(cell.id * 13.7);
    let d = cell.f1 / size;
    let eps = 0.02;
    let slope = (crater_height(d + eps) - crater_height(d - eps)) / (2.0 * eps);
    let radial = p - cell.point;
    let tangent = radial - n * glm::dot(&radial, n);
    if tangent.norm() > 1e-5 {
        out.bump = glm::normalize(&tangent) * (slope * strength * scale / size);
    }
    out.floor = smoothstep(1.0, 0.6, d);
    out.rim = (-((d - 1.0) / 0.12).powi(2)).exp();
    // Solo los cráteres más jóvenes (id alto) muestran rayos de eyección.
    if cell.id > 0.86 {
        let angle_noise = value_noise(&(radial * 9.0 / size + offset));
        out.ejecta = smoothstep(2.8, 1.0, d) * smoothstep(0.35, 0.8, angle_noise) * smoothstep(0.9, 1.1, d);
    }
    out
}

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();

    // CAPA 1: variación de albedo del regolito (gris cálido/frío con fbm).
    let albedo = fbm(&(n * 4.0 + v3(3.0, 1.0, 2.0)), 4);
    let fine = fbm(&(n * 26.0), 2);
    let mut color = mix3(&rgb(0x8d8a84), &rgb(0xb3b0a8), smoothstep(-0.2, 0.25, albedo));
    color *= 0.92 + 0.25 * fine;

    // CAPA 2: maria (llanuras de basalto oscuro) por fbm de baja frecuencia.
    let maria = fbm(&(warp(&(n * 1.2), 0.4, 2) + v3(9.0, 0.0, 0.0)), 4);
    let mare = smoothstep(0.02, 0.12, maria);
    color = mix3(&color, &(rgb(0x45464c) * (0.9 + 0.3 * fine)), mare * 0.85);

    // CAPA 3: cráteres por voronoi en dos escalas (fondo oscuro, borde brillante, relieve).
    let big = crater_layer(&n, 4.0, v3(0.0, 0.0, 0.0), 0.25, 0.020);
    let small = crater_layer(&n, 11.0, v3(17.0, 4.0, 9.0), 0.40, 0.012);
    for c in [&big, &small] {
        color *= 1.0 - 0.28 * c.floor;
        color = mix3(&color, &rgb(0xd8d5cc), c.rim * 0.55);
    }

    // CAPA 4: polvo claro de eyecciones (rayos alrededor de cráteres jóvenes).
    let ejecta = big.ejecta.max(small.ejecta * 0.7);
    color = mix3(&color, &rgb(0xe6e3da), ejecta * 0.6);

    // Iluminación en espacio de objeto con la normal perturbada por los cráteres.
    let bumped = glm::normalize(&(n - big.bump - small.bump));
    let l = f.light_obj();
    let lambert = glm::dot(&bumped, &l).max(0.0);
    // Sin atmósfera: terminador nítido, sombra de Terra (eclipse) y poca luz ambiental.
    let shadow = f.external_shadow();
    let lit = lambert * shadow * smoothstep(-0.02, 0.08, glm::dot(&n, &l));
    opaque(color * (0.018 + 1.1 * lit))
}
