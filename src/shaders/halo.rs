//! Halo atmosférico: se dibuja sobre una cáscara esférica más grande que el
//! planeta, solo con sus caras traseras y mezcla aditiva. La intensidad
//! depende de la distancia mínima entre el rayo de vista y el centro del
//! planeta, lo que produce un resplandor que se desvanece hacia afuera.

use super::utils::*;
use super::{FragmentIn, Halo};
use nalgebra_glm as glm;

/// Distancia (en radios del planeta) a la que el rayo de vista pasa del centro.
/// Devuelve también el punto de máxima aproximación.
pub fn ray_closest(f: &FragmentIn) -> (f32, Vec3) {
    let dir = glm::normalize(&(f.pos_world - f.camera_pos));
    let oc = f.ctx.center - f.camera_pos;
    let t = glm::dot(&oc, &dir);
    let q = f.camera_pos + dir * t;
    ((q - f.ctx.center).norm() / f.ctx.radius, q)
}

pub fn shade(f: &FragmentIn, h: &Halo) -> Option<Vec4> {
    let (d, q) = ray_closest(f);
    let x = saturate((d - 1.0) / (h.shell - 1.0));
    if x >= 1.0 {
        return None;
    }
    // Perfil de densidad: denso junto al limbo, se desvanece hacia afuera.
    let glow = (1.0 - x).powf(2.6);
    // El lado iluminado de la atmósfera dispersa más luz.
    let side = glm::dot(&glm::normalize(&(q - f.ctx.center)), &f.light_dir);
    let lit = 0.12 + 0.88 * smoothstep(-0.45, 0.55, side);
    Some(Vec4::new(h.color.x, h.color.y, h.color.z, glow * lit * h.intensity))
}
