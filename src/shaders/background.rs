//! Fondo procedural: campo de estrellas con centelleo y nebulosa tenue.
//!
//! La parte estática (nebulosa y posición de las estrellas) depende solo de
//! la dirección de vista, por lo que el pipeline la guarda en caché mientras
//! la cámara no se mueve; cada cuadro solo se anima el centelleo.

use super::utils::*;

/// Contribución estática del fondo en una dirección de vista unitaria.
pub struct BackgroundSample {
    /// Nebulosa (color de fondo).
    pub nebula: Vec3,
    /// Color de la estrella (ya multiplicado por su brillo); cero si no hay estrella.
    pub star: Vec3,
    /// Fase aleatoria del centelleo en [0, 1).
    pub phase: f32,
}

pub fn sample(dir: &Vec3) -> BackgroundSample {
    // Nebulosa: fbm de baja frecuencia con dos tintes fríos.
    let neb = fbm(&(dir * 2.2 + v3(3.1, 0.0, 1.7)), 3);
    let neb2 = fbm(&(dir * 4.5 + v3(-2.0, 5.0, 0.3)), 2);
    let nebula = (rgb(0x1a0b3a) * smoothstep(-0.05, 0.35, neb) * 0.55
        + rgb(0x06223a) * smoothstep(0.0, 0.3, neb2) * 0.45)
        * 0.35;

    // Estrellas: una por celda 3D sobre la esfera de direcciones, en dos escalas.
    let mut star = Vec3::zeros();
    let mut phase = 0.0;
    for (scale, density, size) in [(140.0, 0.93, 0.13), (60.0, 0.97, 0.10)] {
        let p = dir * scale;
        let (cx, cy, cz) = (ifloor(p.x), ifloor(p.y), ifloor(p.z));
        let h = hash13(cx, cy, cz);
        if h > density {
            let jitter = v3(hash13(cx, 11, cz), hash13(cy, cz, 3), hash13(cx, cy, 5));
            let center = v3(cx as f32, cy as f32, cz as f32) + v3(0.2, 0.2, 0.2) + jitter * 0.6;
            let d = (p - center).norm();
            let brightness = (h - density) / (1.0 - density) * smoothstep(size * 2.5, 0.0, d) * 1.6;
            let tint = mix3(&rgb(0xbfd4ff), &rgb(0xffe2c0), jitter.x);
            star += tint * brightness;
            phase = h;
        }
    }
    BackgroundSample { nebula, star, phase }
}

/// Factor de centelleo de una estrella según su fase y el tiempo.
#[inline]
pub fn twinkle(phase: f32, time: f32) -> f32 {
    0.75 + 0.25 * (time * (2.0 + 6.0 * phase) + phase * 50.0).sin()
}
