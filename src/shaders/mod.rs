//! Interfaz uniforme de shaders y despacho por cuerpo celeste.
//!
//! Cada cuerpo tiene su propio archivo con una función
//! `shade(&FragmentIn) -> Option<Vec4>`: devuelve RGB + alfa, o `None` para
//! descartar el fragmento. Ningún shader usa texturas: todo el color sale de
//! funciones matemáticas (ver `utils.rs`).

pub mod background;
pub mod utils;

mod aurelia;
mod cristalis;
mod halo;
mod helios;
mod ignis;
mod nyx;
pub mod rings;
mod selene;
mod terra;

use nalgebra_glm as glm;
pub use rings::RingStyle;
use utils::{Vec3, Vec4};

/// Datos de entrada de cada fragmento (interpolados por el rasterizador).
pub struct FragmentIn<'a> {
    /// Posición en espacio de objeto (dominio del ruido: sin costuras).
    pub pos_obj: Vec3,
    /// Normal interpolada en espacio de mundo (normalizada).
    pub normal: Vec3,
    /// Coordenadas UV (solo como dato numérico, nunca como textura).
    pub uv: glm::Vec2,
    /// Posición en espacio de mundo.
    pub pos_world: Vec3,
    /// Tiempo de animación en segundos.
    pub time: f32,
    /// Dirección hacia la luz (el sol), en mundo.
    pub light_dir: Vec3,
    /// Dirección hacia la cámara, en mundo.
    pub view_dir: Vec3,
    pub camera_pos: Vec3,
    /// Datos propios de la llamada de dibujo.
    pub ctx: &'a ShadeContext,
}

impl FragmentIn<'_> {
    /// Normal unitaria en espacio de objeto (posición sobre la esfera unitaria).
    #[inline]
    pub fn n_obj(&self) -> Vec3 {
        glm::normalize(&self.pos_obj)
    }

    /// Dirección de la luz expresada en espacio de objeto.
    #[inline]
    pub fn light_obj(&self) -> Vec3 {
        self.ctx.world_to_obj * self.light_dir
    }

    /// Sombra de cuerpos externos: eclipse de la luna/planeta y sombra de anillos.
    pub fn external_shadow(&self) -> f32 {
        let mut s = 1.0;
        if let Some((center, radius)) = self.ctx.occluder {
            s *= utils::sphere_shadow(&self.pos_world, &self.light_dir, &center, radius);
        }
        if let Some(ring) = &self.ctx.ring_shadow {
            s *= rings::shadow_on_planet(ring, &self.pos_world, &self.light_dir);
        }
        s
    }
}

/// Anillo que proyecta sombra sobre su planeta.
#[derive(Clone, Copy, Debug)]
pub struct RingShadow {
    pub center: Vec3,
    pub normal: Vec3,
    /// Radio del planeta en mundo (los radios del estilo son relativos a él).
    pub planet_radius: f32,
    pub style: RingStyle,
}

/// Contexto por llamada de dibujo (equivalente a "uniforms" del cuerpo).
#[derive(Clone, Copy, Debug)]
pub struct ShadeContext {
    /// Centro del cuerpo en mundo.
    pub center: Vec3,
    /// Radio del cuerpo en mundo.
    pub radius: f32,
    /// Rotación mundo -> objeto (para llevar la luz al espacio del ruido).
    pub world_to_obj: glm::Mat3,
    /// Esfera que puede proyectar sombra sobre este cuerpo.
    pub occluder: Option<(Vec3, f32)>,
    /// Anillo que proyecta sombra sobre este planeta.
    pub ring_shadow: Option<RingShadow>,
}

impl ShadeContext {
    pub fn new(center: Vec3, radius: f32, world_to_obj: glm::Mat3) -> Self {
        Self { center, radius, world_to_obj, occluder: None, ring_shadow: None }
    }
}

/// Parámetros de un halo atmosférico (cáscara aditiva alrededor del planeta).
#[derive(Clone, Copy, Debug)]
pub struct Halo {
    pub color: Vec3,
    /// Radio de la cáscara relativo al planeta (p. ej. 1.12).
    pub shell: f32,
    pub intensity: f32,
}

/// Shader asociado a cada llamada de dibujo.
#[derive(Clone, Copy, Debug)]
pub enum ShaderKind {
    Helios,
    Corona { shell: f32 },
    Terra,
    Selene,
    Aurelia,
    Ignis,
    Cristalis,
    Nyx,
    Rings(RingStyle),
    Halo(Halo),
}

#[inline]
pub fn opaque(c: Vec3) -> Option<Vec4> {
    Some(Vec4::new(c.x, c.y, c.z, 1.0))
}

/// Despacha el fragment shader correspondiente.
pub fn shade(kind: &ShaderKind, f: &FragmentIn) -> Option<Vec4> {
    match kind {
        ShaderKind::Helios => helios::shade(f),
        ShaderKind::Corona { shell } => helios::corona(f, *shell),
        ShaderKind::Terra => terra::shade(f),
        ShaderKind::Selene => selene::shade(f),
        ShaderKind::Aurelia => aurelia::shade(f),
        ShaderKind::Ignis => ignis::shade(f),
        ShaderKind::Cristalis => cristalis::shade(f),
        ShaderKind::Nyx => nyx::shade(f),
        ShaderKind::Rings(style) => rings::shade(f, style),
        ShaderKind::Halo(h) => halo::shade(f, h),
    }
}
