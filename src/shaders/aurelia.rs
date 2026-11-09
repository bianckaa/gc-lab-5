//! GIGANTE GASEOSO "Aurelia" (tipo Júpiter/Saturno).

use super::utils::*;
use super::{opaque, FragmentIn};
use nalgebra_glm as glm;
use std::f32::consts::{PI, TAU};

/// Rampa latitudinal de polo sur (0) a polo norte (1): crema, ocre, óxido y marrón.
const BANDS: [Stop; 15] = [
    (0.00, [0.46, 0.40, 0.38]),
    (0.10, [0.62, 0.48, 0.36]),
    (0.17, [0.84, 0.66, 0.44]),
    (0.24, [0.56, 0.30, 0.17]),
    (0.30, [0.93, 0.84, 0.66]),
    (0.36, [0.72, 0.42, 0.22]),
    (0.42, [0.97, 0.90, 0.74]),
    (0.50, [0.99, 0.94, 0.80]),
    (0.57, [0.87, 0.64, 0.38]),
    (0.63, [0.48, 0.27, 0.15]),
    (0.69, [0.94, 0.85, 0.68]),
    (0.76, [0.74, 0.48, 0.27]),
    (0.83, [0.90, 0.76, 0.56]),
    (0.91, [0.60, 0.47, 0.37]),
    (1.00, [0.44, 0.39, 0.38]),
];

/// Posición de la Gran Tormenta (latitud como seno, longitud en radianes).
const STORM_LAT: f32 = 0.27;
const STORM_LON: f32 = 0.9;

/// Velocidad de los vientos zonales (rad/s) según la latitud.
#[inline]
fn wind(lat: f32) -> f32 {
    0.05 * (lat * 13.0).sin() + 0.015
}

/// Periodo del "flow map": cada capa de turbulencia se reinicia tras este tiempo
/// para que la cizalladura de los vientos no estire el patrón sin límite.
const FLOW_PERIOD: f32 = 30.0;

/// Turbulencia y remolinos evaluados en el dominio desplazado por el viento.
fn turbulence_at(n: &Vec3, advance: f32) -> (f32, f32) {
    let p = rotate_y(n, advance * wind(n.y));
    let stretched = p.component_mul(&v3(2.2, 7.5, 2.2));
    let turbulence = fbm(&warp(&stretched, 0.9, 3), 4);
    let eddies = fbm(&(warp(&(p * 5.0), 0.5, 2) * 2.0), 3);
    (turbulence, eddies)
}

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;
    let lat = n.y;

    // CAPA 2: turbulencia en los bordes de las bandas (domain warping sobre sin(latitud)).
    // Rotación diferencial (vientos zonales) con dos capas desfasadas que se
    // mezclan de forma cíclica: el flujo se ve continuo y nunca se degrada.
    let phase_a = fract(t / FLOW_PERIOD);
    let phase_b = fract(t / FLOW_PERIOD + 0.5);
    let (turb_a, eddy_a) = turbulence_at(&n, phase_a * FLOW_PERIOD);
    let (turb_b, eddy_b) = turbulence_at(&n, phase_b * FLOW_PERIOD);
    let blend = (2.0 * phase_a - 1.0).abs();
    let turbulence = mix(turb_a, turb_b, blend);
    let eddies = mix(eddy_a, eddy_b, blend);
    let edge = 1.0 - (lat * PI * 4.6).sin().abs();
    let band_coord = lat + turbulence * (0.025 + 0.075 * edge) + eddies * 0.012;
    // La tormenta y los óvalos derivan como un bloque rígido con el viento de su latitud.
    let p = rotate_y(&n, t * wind(STORM_LAT));

    // CAPA 1: bandas latitudinales con rampa crema/ocre/óxido/marrón y estrías finas.
    let mut color = ramp(&BANDS, band_coord * 0.5 + 0.5);
    let streaks = 0.5 + 0.5 * (band_coord * 95.0 + turbulence * 6.0).sin();
    color *= 0.93 + 0.10 * streaks;
    color = mix3(&color, &rgb(0xfff3dc), smoothstep(0.12, 0.3, eddies) * edge * 0.35);

    // CAPA 3: Gran Tormenta (óvalo con vórtice en espiral) y óvalos blancos menores.
    let lon = p.z.atan2(p.x);
    let mut dlon = lon - STORM_LON;
    dlon -= TAU * (dlon / TAU).round();
    let dx = dlon * (1.0 - STORM_LAT * STORM_LAT).sqrt() / 0.36;
    let dy = (lat - STORM_LAT) / 0.13;
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1.6 {
        let angle = dy.atan2(dx) + (1.0 - d).max(0.0) * 4.5 - t * 0.6;
        let swirl_noise = fbm(&v3(dx * 2.0, dy * 2.0, t * 0.05), 3);
        let spiral = 0.5 + 0.5 * (angle * 2.0 + d * 9.0 + swirl_noise * 4.0).sin();
        let storm_color = mix3(
            &mix3(&rgb(0x8f2412), &rgb(0xd0582b), spiral),
            &rgb(0xf3c6a0),
            smoothstep(0.35, 1.0, d),
        );
        let inside = smoothstep(1.05, 0.82, d);
        let collar = (-((d - 1.1) / 0.09).powi(2)).exp();
        color = mix3(&color, &storm_color, inside);
        color = mix3(&color, &rgb(0xfbeedd), collar * 0.55);
    }
    let ovals = voronoi(&p.component_mul(&v3(5.0, 13.0, 5.0)));
    let oval_band = smoothstep(0.08, 0.0, (lat - 0.48).abs()) + smoothstep(0.06, 0.0, (lat + 0.62).abs());
    let white_oval = smoothstep(0.32, 0.12, ovals.f1) * smoothstep(0.8, 0.85, ovals.id) * oval_band;
    color = mix3(&color, &rgb(0xfffaf0), white_oval.min(1.0) * 0.85);

    // CAPA 4: oscurecimiento polar, limbo atmosférico y terminador suave.
    let polar = smoothstep(0.62, 0.95, lat.abs());
    color = mix3(&color, &rgb(0x55505a), polar * 0.65);
    let mu = glm::dot(&f.normal, &f.view_dir).max(0.0);
    color *= 0.55 + 0.45 * mu.powf(0.45);
    let ndl = glm::dot(&f.normal, &f.light_dir);
    let shadow = f.external_shadow();
    let diffuse = soft_diffuse(&f.normal, &f.light_dir, 0.28) * shadow;
    let terminator = smoothstep(-0.15, 0.05, ndl) * smoothstep(0.35, 0.05, ndl);
    let mut lit = color * (0.02 + 1.05 * diffuse);
    lit += rgb(0x6e2a0c) * terminator * 0.25 * shadow;
    lit += rgb(0xffd9a0) * fresnel(&f.normal, &f.view_dir, 4.0) * diffuse * 0.25;

    opaque(lit)
}
