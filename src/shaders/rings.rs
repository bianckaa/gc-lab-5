//! SISTEMAS DE ANILLOS (modelo separado: annulus plano generado en código).
//! Se usan en Aurelia (anillos dorados anchos) y en Cristalis (anillos de hielo finos).

use super::utils::*;
use super::{FragmentIn, RingShadow};
use nalgebra_glm as glm;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RingPalette {
    Golden,
    Icy,
}

/// Geometría y estilo de un sistema de anillos (radios relativos al planeta).
#[derive(Clone, Copy, Debug)]
pub struct RingStyle {
    pub inner: f32,
    pub outer: f32,
    pub palette: RingPalette,
}

pub const AURELIA_RINGS: RingStyle = RingStyle { inner: 1.32, outer: 2.38, palette: RingPalette::Golden };
pub const CRISTALIS_RINGS: RingStyle = RingStyle { inner: 1.55, outer: 1.98, palette: RingPalette::Icy };

/// Opacidad del anillo según la posición radial normalizada rn en [0, 1].
/// Combina CAPA 1 (bandas finas) y CAPA 2 (divisiones tipo Cassini).
pub fn density(style: &RingStyle, rn: f32) -> f32 {
    if !(0.0..=1.0).contains(&rn) {
        return 0.0;
    }
    // CAPA 1: bandas radiales finas (ruido 1D a varias frecuencias).
    let fine = 0.55 * value_noise_1d(rn * 70.0, 1) + 0.30 * value_noise_1d(rn * 260.0, 2) + 0.15 * value_noise_1d(rn * 900.0, 3);
    let edges = smoothstep(0.0, 0.03, rn) * smoothstep(1.0, 0.97, rn);
    match style.palette {
        RingPalette::Golden => {
            // Estructura C / B / división de Cassini / A con hueco de Encke.
            let c_ring = 0.22 * smoothstep(0.0, 0.16, rn);
            let b_ring = 0.95 * smoothstep(0.17, 0.22, rn) * smoothstep(0.60, 0.585, rn);
            let a_ring = 0.70 * smoothstep(0.655, 0.675, rn);
            let base = if rn < 0.2 { c_ring.max(b_ring) } else { b_ring.max(a_ring) };
            // CAPA 2: divisiones tipo Cassini (huecos casi vacíos).
            let cassini = smoothstep(0.585, 0.60, rn) * smoothstep(0.672, 0.655, rn);
            let encke = smoothstep(0.865, 0.872, rn) * smoothstep(0.888, 0.880, rn);
            let keeler = smoothstep(0.955, 0.958, rn) * smoothstep(0.966, 0.962, rn);
            let gaps = 1.0 - (cassini * 0.97).max(encke).max(keeler * 0.9);
            base * (0.55 + 0.55 * fine) * gaps * edges
        }
        RingPalette::Icy => {
            // Dos anillos finos separados por un hueco ancho, más un anillo exterior tenue.
            let inner_ring = smoothstep(0.0, 0.05, rn) * smoothstep(0.42, 0.38, rn);
            let outer_ring = smoothstep(0.56, 0.60, rn) * smoothstep(0.80, 0.76, rn);
            let wisp = 0.35 * smoothstep(0.86, 0.88, rn) * smoothstep(0.95, 0.92, rn);
            (inner_ring * 0.75 + outer_ring * 0.9 + wisp) * (0.45 + 0.7 * fine) * edges
        }
    }
}

pub fn shade(f: &FragmentIn, style: &RingStyle) -> Option<Vec4> {
    let r = (f.pos_obj.x * f.pos_obj.x + f.pos_obj.z * f.pos_obj.z).sqrt();
    let rn = (r - style.inner) / (style.outer - style.inner);
    // La coordenada v del annulus es el ángulo normalizado (dato numérico, no textura).
    let angle = f.uv.y * std::f32::consts::TAU;

    let alpha = density(style, rn);
    // CAPA 2: los huecos se descartan (el fondo y el planeta se ven a través).
    if alpha < 0.03 {
        return None;
    }

    // CAPA 3: variación de color y brillo por ruido radial + "radios" angulares tenues.
    let tint = value_noise_1d(rn * 35.0, 9);
    let spokes = value_noise(&v3(angle * 6.0 + f.time * 0.05, rn * 8.0, 0.0));
    let mut color = match style.palette {
        RingPalette::Golden => {
            const GOLD: [Stop; 5] = [
                (0.00, [0.48, 0.42, 0.36]),
                (0.20, [0.80, 0.70, 0.55]),
                (0.45, [0.96, 0.88, 0.70]),
                (0.70, [0.84, 0.74, 0.58]),
                (1.00, [0.70, 0.62, 0.52]),
            ];
            ramp(&GOLD, rn) * (0.82 + 0.3 * tint)
        }
        RingPalette::Icy => {
            const ICE: [Stop; 3] = [
                (0.00, [0.62, 0.80, 0.95]),
                (0.60, [0.86, 0.95, 1.00]),
                (1.00, [0.70, 0.78, 1.00]),
            ];
            ramp(&ICE, rn) * (0.85 + 0.3 * tint)
        }
    };
    color *= 0.9 + 0.18 * spokes;

    // CAPA 4: iluminación (difusión hacia ambos lados) y sombra del planeta sobre el anillo.
    let ndl = glm::dot(&f.normal, &f.light_dir).abs();
    let forward_scatter = glm::dot(&f.view_dir, &-f.light_dir).max(0.0).powf(6.0) * 0.6;
    let shadow = match f.ctx.occluder {
        Some((c, rad)) => sphere_shadow(&f.pos_world, &f.light_dir, &c, rad),
        None => 1.0,
    };
    let light = (0.25 + 0.85 * ndl.sqrt() + forward_scatter) * shadow + 0.03;
    if style.palette == RingPalette::Icy {
        // Destello cristalino en los anillos de hielo.
        let h = glm::normalize(&(f.light_dir + f.view_dir));
        color += v3(0.7, 0.85, 1.0) * glm::dot(&f.normal, &h).abs().powf(60.0) * shadow * 0.8;
    }

    let c = color * light;
    Some(Vec4::new(c.x, c.y, c.z, (alpha * 1.05).min(0.97)))
}

/// Sombra de los anillos sobre el planeta: se intersecta el rayo hacia la luz
/// con el plano del anillo y se evalúa su densidad en ese radio.
pub fn shadow_on_planet(ring: &RingShadow, p: &Vec3, l: &Vec3) -> f32 {
    let denom = glm::dot(l, &ring.normal);
    if denom.abs() < 1e-4 {
        return 1.0;
    }
    let t = glm::dot(&(ring.center - p), &ring.normal) / denom;
    if t <= 0.0 {
        return 1.0;
    }
    let q = p + l * t;
    let r = (q - ring.center).norm() / ring.planet_radius;
    let rn = (r - ring.style.inner) / (ring.style.outer - ring.style.inner);
    1.0 - 0.8 * density(&ring.style, rn).min(1.0)
}
