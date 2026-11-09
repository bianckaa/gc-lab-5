//! Utilidades reutilizables para los shaders procedurales.
//!
//! Todo el color del laboratorio sale de estas funciones matemáticas:
//! hash, ruido de valor y de gradiente 3D, fbm, fbm "ridged", voronoi
//! (F1, F2 y F2-F1), domain warping, rampas de color, fresnel e iluminación
//! difusa con terminador suave. El ruido se evalúa sobre la posición 3D en
//! espacio de objeto, por lo que no existen costuras de UV ni distorsión en
//! los polos.

use nalgebra_glm as glm;

pub type Vec3 = glm::Vec3;
pub type Vec4 = glm::Vec4;

// ---------------------------------------------------------------------------
// Ayudantes escalares y vectoriales
// ---------------------------------------------------------------------------

#[inline(always)]
pub fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Convierte un color hexadecimal 0xRRGGBB a un vector RGB en [0, 1].
#[inline]
pub fn rgb(hex: u32) -> Vec3 {
    v3(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

#[inline(always)]
pub fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[inline(always)]
pub fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline(always)]
pub fn mix3(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[inline(always)]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = saturate((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

/// Piso rápido: `f32::floor` se compila como llamada a libm en x86-64 sin
/// SSE4.1; esta versión usa una conversión entera y es varias veces más rápida.
#[inline(always)]
pub fn ffloor(x: f32) -> f32 {
    ifloor(x) as f32
}

#[inline(always)]
pub fn ifloor(x: f32) -> i32 {
    let i = x as i32;
    if x < i as f32 {
        i - 1
    } else {
        i
    }
}

#[inline(always)]
pub fn fract(x: f32) -> f32 {
    x - ffloor(x)
}

/// Rota un punto alrededor del eje Y (útil para animar nubes y vientos).
#[inline]
pub fn rotate_y(p: &Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    v3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z)
}

// ---------------------------------------------------------------------------
// Hash entero (sin tablas, determinista y rápido)
// ---------------------------------------------------------------------------

/// Hash de celda: una sola ronda multiplicativa (suficiente para ruido y
/// mucho más barata que una mezcla completa en el bucle interno).
#[inline(always)]
fn hash_u32(x: i32, y: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^ (h >> 12)
}

#[inline(always)]
fn to_unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Hash de una celda entera a un flotante en [0, 1).
#[inline]
pub fn hash13(x: i32, y: i32, z: i32) -> f32 {
    to_unit(hash_u32(x, y, z))
}

/// Hash de una celda entera a tres flotantes en [0, 1).
#[inline]
fn hash33(x: i32, y: i32, z: i32) -> (f32, f32, f32) {
    let h = hash_u32(x, y, z);
    (
        to_unit(h),
        to_unit(h.wrapping_mul(0x9e37_79b1)),
        to_unit(h.wrapping_mul(0x85eb_ca77)),
    )
}

/// Hash de un punto continuo (se discretiza con la escala indicada).
#[inline]
pub fn hash_point(p: &Vec3, scale: f32) -> f32 {
    hash13(
        ifloor(p.x * scale),
        ifloor(p.y * scale),
        ifloor(p.z * scale),
    )
}

// ---------------------------------------------------------------------------
// Ruido de valor y de gradiente
// ---------------------------------------------------------------------------

#[inline(always)]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Gradientes de Perlin mejorado (12 direcciones sobre aristas del cubo,
/// cuatro repetidas para completar 16). Tabla sin ramas: evita fallos de
/// predicción en el bucle interno del ruido.
const GRADS: [[f32; 3]; 16] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0], [0.0, -1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0], [0.0, -1.0, 1.0], [-1.0, 1.0, 0.0], [0.0, -1.0, -1.0],
];

#[inline(always)]
fn grad(h: u32, x: f32, y: f32, z: f32) -> f32 {
    let g = &GRADS[(h & 15) as usize];
    g[0] * x + g[1] * y + g[2] * z
}

/// Ruido de gradiente 3D (tipo Perlin). Rango aproximado [-1, 1].
pub fn noise(p: &Vec3) -> f32 {
    let (ix, iy, iz) = (ifloor(p.x), ifloor(p.y), ifloor(p.z));
    let (fx, fy, fz) = (ix as f32, iy as f32, iz as f32);
    let (x, y, z) = (p.x - fx, p.y - fy, p.z - fz);
    let (u, v, w) = (fade(x), fade(y), fade(z));

    let n000 = grad(hash_u32(ix, iy, iz), x, y, z);
    let n100 = grad(hash_u32(ix + 1, iy, iz), x - 1.0, y, z);
    let n010 = grad(hash_u32(ix, iy + 1, iz), x, y - 1.0, z);
    let n110 = grad(hash_u32(ix + 1, iy + 1, iz), x - 1.0, y - 1.0, z);
    let n001 = grad(hash_u32(ix, iy, iz + 1), x, y, z - 1.0);
    let n101 = grad(hash_u32(ix + 1, iy, iz + 1), x - 1.0, y, z - 1.0);
    let n011 = grad(hash_u32(ix, iy + 1, iz + 1), x, y - 1.0, z - 1.0);
    let n111 = grad(hash_u32(ix + 1, iy + 1, iz + 1), x - 1.0, y - 1.0, z - 1.0);

    let x00 = mix(n000, n100, u);
    let x10 = mix(n010, n110, u);
    let x01 = mix(n001, n101, u);
    let x11 = mix(n011, n111, u);
    mix(mix(x00, x10, v), mix(x01, x11, v), w)
}

/// Ruido de valor 3D. Rango [0, 1].
pub fn value_noise(p: &Vec3) -> f32 {
    let (ix, iy, iz) = (ifloor(p.x), ifloor(p.y), ifloor(p.z));
    let (fx, fy, fz) = (ix as f32, iy as f32, iz as f32);
    let (u, v, w) = (fade(p.x - fx), fade(p.y - fy), fade(p.z - fz));
    let c = |dx: i32, dy: i32, dz: i32| hash13(ix + dx, iy + dy, iz + dz);
    let x00 = mix(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = mix(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = mix(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = mix(c(0, 1, 1), c(1, 1, 1), u);
    mix(mix(x00, x10, v), mix(x01, x11, v), w)
}

/// Ruido de valor 1D (para anillos y bandas radiales). Rango [0, 1].
pub fn value_noise_1d(x: f32, seed: i32) -> f32 {
    let i = ifloor(x);
    let t = fade(x - i as f32);
    mix(hash13(i, seed, 7), hash13(i + 1, seed, 7), t)
}

// ---------------------------------------------------------------------------
// Fractales
// ---------------------------------------------------------------------------

/// Desplazamiento entre octavas para romper la alineación con la rejilla.
const OCTAVE_SHIFT: [f32; 3] = [1.73, 9.21, -3.17];

/// Movimiento browniano fraccional. Rango típico [-0.6, 0.6].
pub fn fbm(p: &Vec3, octaves: u32) -> f32 {
    let mut q = *p;
    let mut amp = 0.5;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        sum += amp * noise(&q);
        norm += amp;
        q = q * 2.03 + v3(OCTAVE_SHIFT[0], OCTAVE_SHIFT[1], OCTAVE_SHIFT[2]);
        amp *= 0.5;
    }
    sum / norm
}

/// fbm "ridged": crestas afiladas (montañas, venas, ríos). Rango [0, 1].
pub fn ridged(p: &Vec3, octaves: u32) -> f32 {
    let mut q = *p;
    let mut amp = 0.5;
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut weight = 1.0;
    for _ in 0..octaves {
        let r = 1.0 - noise(&q).abs();
        let r = r * r * weight;
        weight = saturate(r * 1.5);
        sum += amp * r;
        norm += amp;
        q = q * 2.07 + v3(OCTAVE_SHIFT[0], OCTAVE_SHIFT[1], OCTAVE_SHIFT[2]);
        amp *= 0.5;
    }
    sum / norm
}

/// Domain warping: desplaza el dominio con fbm para formas orgánicas.
pub fn warp(p: &Vec3, strength: f32, octaves: u32) -> Vec3 {
    let o = v3(
        fbm(p, octaves),
        fbm(&(p + v3(5.2, 1.3, 2.8)), octaves),
        fbm(&(p + v3(1.7, 9.2, 4.1)), octaves),
    );
    p + o * strength
}

// ---------------------------------------------------------------------------
// Voronoi / ruido celular
// ---------------------------------------------------------------------------

/// Resultado de una consulta voronoi.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// Distancia al punto de característica más cercano (F1).
    pub f1: f32,
    /// Distancia al segundo punto más cercano (F2).
    pub f2: f32,
    /// Identificador aleatorio de la celda más cercana en [0, 1).
    pub id: f32,
    /// Posición del punto de característica más cercano.
    pub point: Vec3,
}

impl Cell {
    /// F2 - F1: cero sobre los bordes de las celdas (grietas, fisuras).
    #[inline]
    pub fn edge(&self) -> f32 {
        self.f2 - self.f1
    }
}

/// Voronoi 3D con búsqueda en la vecindad 3x3x3.
pub fn voronoi(p: &Vec3) -> Cell {
    let (ix, iy, iz) = (ifloor(p.x), ifloor(p.y), ifloor(p.z));
    let mut d1 = f32::MAX;
    let mut d2 = f32::MAX;
    let mut best = (0, 0, 0);
    let mut best_point = *p;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy, cz) = (ix + dx, iy + dy, iz + dz);
                let (jx, jy, jz) = hash33(cx, cy, cz);
                let fp = v3(
                    cx as f32 + 0.1 + 0.8 * jx,
                    cy as f32 + 0.1 + 0.8 * jy,
                    cz as f32 + 0.1 + 0.8 * jz,
                );
                let d = (fp - p).norm_squared();
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    best = (cx, cy, cz);
                    best_point = fp;
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
    }
    Cell {
        f1: d1.sqrt(),
        f2: d2.sqrt(),
        id: hash13(best.0 + 31, best.1 - 17, best.2 + 5),
        point: best_point,
    }
}

// ---------------------------------------------------------------------------
// Color e iluminación
// ---------------------------------------------------------------------------

/// Parada de una rampa de color: (posición, [r, g, b]).
pub type Stop = (f32, [f32; 3]);

/// Evalúa una rampa de color (gradient ramp) con interpolación suave.
pub fn ramp(stops: &[Stop], t: f32) -> Vec3 {
    let first = stops[0];
    if t <= first.0 {
        return v3(first.1[0], first.1[1], first.1[2]);
    }
    for pair in stops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if t <= b.0 {
            let k = smoothstep(a.0, b.0, t);
            return v3(
                mix(a.1[0], b.1[0], k),
                mix(a.1[1], b.1[1], k),
                mix(a.1[2], b.1[2], k),
            );
        }
    }
    let last = stops[stops.len() - 1];
    v3(last.1[0], last.1[1], last.1[2])
}

/// Término de Fresnel (Schlick simplificado): 0 al centro, 1 en el borde.
#[inline]
pub fn fresnel(n: &Vec3, v: &Vec3, power: f32) -> f32 {
    (1.0 - glm::dot(n, v).max(0.0)).powf(power)
}

/// Difuso con terminador suave ("wrap lighting").
#[inline]
pub fn soft_diffuse(n: &Vec3, l: &Vec3, wrap: f32) -> f32 {
    saturate((glm::dot(n, l) + wrap) / (1.0 + wrap))
}

/// Factor nocturno: 1 en el lado oscuro, 0 en el lado iluminado.
#[inline]
pub fn night_factor(n: &Vec3, l: &Vec3) -> f32 {
    smoothstep(0.12, -0.18, glm::dot(n, l))
}

/// Especular Blinn-Phong.
#[inline]
pub fn specular(n: &Vec3, l: &Vec3, v: &Vec3, shininess: f32) -> f32 {
    let h = glm::normalize(&(l + v));
    glm::dot(n, &h).max(0.0).powf(shininess) * smoothstep(-0.05, 0.15, glm::dot(n, l))
}

/// Sombra suave proyectada por una esfera (eclipses y sombra del planeta
/// sobre los anillos). Devuelve 1 si está iluminado y 0 en sombra total.
pub fn sphere_shadow(p: &Vec3, l: &Vec3, center: &Vec3, radius: f32) -> f32 {
    let oc = center - p;
    let t = glm::dot(&oc, l);
    if t <= 0.0 {
        return 1.0;
    }
    let d = (oc - l * t).norm();
    let penumbra = radius * 0.04 + t * 0.012;
    smoothstep(radius - penumbra, radius + penumbra, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Imprime percentiles de cada ruido para calibrar umbrales.
    #[test]
    fn noise_ranges() {
        let mut samples: Vec<(&str, Vec<f32>)> = vec![
            ("noise", vec![]),
            ("fbm4", vec![]),
            ("fbm6", vec![]),
            ("ridged5", vec![]),
            ("value", vec![]),
            ("vor_f1", vec![]),
            ("vor_edge", vec![]),
        ];
        for i in 0..20000 {
            let p = v3(
                hash13(i, 1, 2) * 40.0 - 20.0,
                hash13(i, 3, 4) * 40.0 - 20.0,
                hash13(i, 5, 6) * 40.0 - 20.0,
            );
            samples[0].1.push(noise(&p));
            samples[1].1.push(fbm(&p, 4));
            samples[2].1.push(fbm(&p, 6));
            samples[3].1.push(ridged(&p, 5));
            samples[4].1.push(value_noise(&p));
            let c = voronoi(&p);
            samples[5].1.push(c.f1);
            samples[6].1.push(c.edge());
        }
        for (name, mut s) in samples {
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let q = |f: f32| s[((s.len() - 1) as f32 * f) as usize];
            println!(
                "{name:>9}: min {:+.3} p05 {:+.3} p25 {:+.3} p50 {:+.3} p75 {:+.3} p95 {:+.3} max {:+.3}",
                q(0.0), q(0.05), q(0.25), q(0.5), q(0.75), q(0.95), q(1.0)
            );
        }
    }
}

