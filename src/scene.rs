//! Definición de los cuerpos celestes, sus transformaciones animadas
//! (rotación y traslación) y las vistas disponibles.

use crate::camera::OrbitCamera;
use crate::mesh::{self, Mesh};
use crate::obj;
use crate::pipeline::{Blend, Cull, DrawCall};
use crate::shaders::rings::{AURELIA_RINGS, CRISTALIS_RINGS};
use crate::shaders::utils::{rgb, v3, Vec3};
use crate::shaders::{Halo, RingShadow, RingStyle, ShadeContext, ShaderKind};
use nalgebra_glm as glm;
use std::path::{Path, PathBuf};

/// Mallas cargadas una sola vez. Anillos y luna son modelos separados del planeta.
pub struct World {
    /// Esfera base cargada desde `assets/sphere.obj`.
    pub sphere: Mesh,
    /// Icosfera generada en código para la luna Selene.
    pub moon: Mesh,
    /// Annulus generado en código para cada sistema de anillos.
    pub ring_aurelia: Mesh,
    pub ring_cristalis: Mesh,
}

fn sphere_path() -> PathBuf {
    let local = Path::new("assets").join("sphere.obj");
    if local.exists() {
        local
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("sphere.obj")
    }
}

impl World {
    pub fn load() -> Result<Self, String> {
        let ring = |s: RingStyle| mesh::annulus(s.inner, s.outer, 256, 3);
        Ok(Self {
            sphere: obj::load_obj(&sphere_path())?,
            moon: mesh::icosphere(4),
            ring_aurelia: ring(AURELIA_RINGS),
            ring_cristalis: ring(CRISTALIS_RINGS),
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Body {
    Helios,
    Terra,
    Aurelia,
    Ignis,
    Cristalis,
    Nyx,
}

impl Body {
    pub const ALL: [Body; 6] = [Body::Helios, Body::Terra, Body::Aurelia, Body::Ignis, Body::Cristalis, Body::Nyx];

    pub fn name(self) -> &'static str {
        match self {
            Body::Helios => "Helios Ardens",
            Body::Terra => "Terra Viridis + Selene",
            Body::Aurelia => "Aurelia",
            Body::Ignis => "Ignis",
            Body::Cristalis => "Cristalis",
            Body::Nyx => "Nyx Lumina",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Body(Body),
    System,
}

impl View {
    pub fn name(self) -> &'static str {
        match self {
            View::Body(b) => b.name(),
            View::System => "Sistema completo",
        }
    }

    /// Nombre de archivo para las capturas (screenshots/<nombre>.png).
    pub fn file_name(self) -> &'static str {
        match self {
            View::Body(Body::Helios) => "1_helios_ardens",
            View::Body(Body::Terra) => "2_terra_viridis",
            View::Body(Body::Aurelia) => "3_aurelia",
            View::Body(Body::Ignis) => "4_ignis",
            View::Body(Body::Cristalis) => "5_cristalis",
            View::Body(Body::Nyx) => "6_nyx_lumina",
            View::System => "0_sistema_completo",
        }
    }
}

/// Parámetros físicos y visuales de cada cuerpo.
struct BodyDef {
    shader: ShaderKind,
    /// Inclinación del eje de rotación (radianes).
    tilt: f32,
    /// Velocidad de rotación propia (rad/s).
    spin: f32,
    /// Radio en la vista de sistema.
    system_radius: f32,
    /// Radio orbital y velocidad angular alrededor del sol (vista de sistema).
    orbit: f32,
    orbit_speed: f32,
    phase: f32,
    halo: Option<Halo>,
    rings: Option<RingStyle>,
    moon: bool,
}

fn def(body: Body) -> BodyDef {
    let halo = |hex: u32, shell: f32, intensity: f32| Some(Halo { color: rgb(hex), shell, intensity });
    match body {
        Body::Helios => BodyDef {
            shader: ShaderKind::Helios, tilt: 0.12, spin: 0.04, system_radius: 2.4,
            orbit: 0.0, orbit_speed: 0.0, phase: 0.0, halo: None, rings: None, moon: false,
        },
        Body::Ignis => BodyDef {
            shader: ShaderKind::Ignis, tilt: 0.15, spin: 0.08, system_radius: 0.6,
            orbit: 4.9, orbit_speed: 0.20, phase: 0.3, halo: halo(0xff5214, 1.09, 0.65), rings: None, moon: false,
        },
        Body::Terra => BodyDef {
            shader: ShaderKind::Terra, tilt: 0.41, spin: 0.10, system_radius: 0.85,
            orbit: 7.6, orbit_speed: 0.13, phase: 2.1, halo: halo(0x5a9dff, 1.11, 0.95), rings: None, moon: true,
        },
        Body::Cristalis => BodyDef {
            shader: ShaderKind::Cristalis, tilt: 0.55, spin: 0.07, system_radius: 0.7,
            orbit: 10.8, orbit_speed: 0.09, phase: 4.0, halo: halo(0x9fe6ff, 1.07, 0.55), rings: Some(CRISTALIS_RINGS), moon: false,
        },
        Body::Aurelia => BodyDef {
            shader: ShaderKind::Aurelia, tilt: 0.45, spin: 0.16, system_radius: 1.4,
            orbit: 15.0, orbit_speed: 0.06, phase: 1.0, halo: halo(0xffd29a, 1.05, 0.35), rings: Some(AURELIA_RINGS), moon: false,
        },
        Body::Nyx => BodyDef {
            shader: ShaderKind::Nyx, tilt: 0.25, spin: 0.09, system_radius: 1.0,
            orbit: 19.5, orbit_speed: 0.04, phase: 5.6, halo: halo(0x22f0ff, 1.17, 1.0), rings: None, moon: false,
        },
    }
}

/// Cámara inicial, límites de zoom y tiempo usado en las capturas.
pub struct ViewPreset {
    pub camera: OrbitCamera,
    pub min_distance: f32,
    pub max_distance: f32,
    pub capture_time: f32,
}

pub fn preset(view: View) -> ViewPreset {
    let p = |yaw: f32, pitch: f32, dist: f32, min: f32, max: f32, time: f32| ViewPreset {
        camera: OrbitCamera::new(yaw, pitch, dist),
        min_distance: min,
        max_distance: max,
        capture_time: time,
    };
    match view {
        View::Body(Body::Helios) => p(0.0, 0.12, 4.9, 2.2, 14.0, 12.0),
        View::Body(Body::Terra) => p(-0.15, 0.2, 5.6, 1.8, 16.0, 26.3),
        View::Body(Body::Aurelia) => p(0.35, 0.24, 7.0, 3.0, 20.0, 34.0),
        View::Body(Body::Ignis) => p(0.5, 0.15, 3.5, 1.6, 12.0, 8.0),
        View::Body(Body::Cristalis) => p(0.3, 0.42, 5.4, 2.2, 14.0, 9.0),
        View::Body(Body::Nyx) => p(0.65, 0.12, 3.6, 1.6, 12.0, 5.0),
        View::System => p(0.4, 0.7, 46.0, 8.0, 90.0, 2.0),
    }
}

/// Dirección de la luz en las vistas individuales (sol lejano a la izquierda).
fn distant_sun() -> Vec3 {
    glm::normalize(&v3(-1.0, 0.3, 0.55)) * 400.0
}

/// Matriz de rotación de un cuerpo: inclinación del eje + giro propio.
fn body_rotation(d: &BodyDef, time: f32) -> glm::Mat4 {
    let tilt = glm::rotation(d.tilt, &v3(0.0, 0.0, 1.0)) * glm::rotation(d.tilt * 0.35, &v3(1.0, 0.0, 0.0));
    tilt * glm::rotation(d.spin * time, &v3(0.0, 1.0, 0.0))
}

fn tilt_only(d: &BodyDef) -> glm::Mat4 {
    glm::rotation(d.tilt, &v3(0.0, 0.0, 1.0)) * glm::rotation(d.tilt * 0.35, &v3(1.0, 0.0, 0.0))
}

fn context(center: Vec3, radius: f32, rotation: &glm::Mat4) -> ShadeContext {
    ShadeContext::new(center, radius, glm::transpose(&glm::mat4_to_mat3(rotation)))
}

/// Listas de dibujo separadas por pasada: opacos, translúcidos y aditivos.
#[derive(Default)]
struct Passes<'a> {
    opaque: Vec<DrawCall<'a>>,
    alpha: Vec<DrawCall<'a>>,
    additive: Vec<DrawCall<'a>>,
}

fn add_body<'a>(passes: &mut Passes<'a>, world: &'a World, body: Body, center: Vec3, radius: f32, time: f32) {
    let d = def(body);
    let rotation = body_rotation(&d, time);
    let model = glm::translation(&center) * rotation * glm::scaling(&v3(radius, radius, radius));
    let mut ctx = context(center, radius, &rotation);

    // Anillos: modelo separado (annulus) con la inclinación del planeta.
    if let Some(style) = d.rings {
        let tilt = tilt_only(&d);
        let ring_normal = glm::normalize(&(glm::mat4_to_mat3(&tilt) * v3(0.0, 1.0, 0.0)));
        ctx.ring_shadow = Some(RingShadow { center, normal: ring_normal, planet_radius: radius, style });
        let mut ring_ctx = context(center, radius, &tilt);
        ring_ctx.occluder = Some((center, radius));
        let mesh = if style.palette == crate::shaders::rings::RingPalette::Golden {
            &world.ring_aurelia
        } else {
            &world.ring_cristalis
        };
        passes.alpha.push(DrawCall {
            mesh,
            model: glm::translation(&center) * tilt * glm::scaling(&v3(radius, radius, radius)),
            shader: ShaderKind::Rings(style),
            ctx: ring_ctx,
            cull: Cull::None,
            blend: Blend::Alpha,
        });
    }

    // Luna Selene: modelo separado (icosfera) en órbita inclinada alrededor de Terra.
    if d.moon {
        let angle = 0.9 + time * 0.22;
        let inclination: f32 = 0.18;
        let orbit = 2.3 * radius;
        let moon_center = center
            + v3(angle.cos() * orbit, angle.sin() * inclination.sin() * orbit, angle.sin() * inclination.cos() * orbit);
        let moon_radius = 0.27 * radius;
        // Rotación sincrónica: siempre muestra la misma cara al planeta.
        let moon_rot = glm::rotation(-angle, &v3(0.0, 1.0, 0.0));
        let mut moon_ctx = context(moon_center, moon_radius, &moon_rot);
        moon_ctx.occluder = Some((center, radius));
        ctx.occluder = Some((moon_center, moon_radius));
        passes.opaque.push(DrawCall {
            mesh: &world.moon,
            model: glm::translation(&moon_center) * moon_rot * glm::scaling(&v3(moon_radius, moon_radius, moon_radius)),
            shader: ShaderKind::Selene,
            ctx: moon_ctx,
            cull: Cull::Back,
            blend: Blend::Opaque,
        });
    }

    passes.opaque.push(DrawCall { mesh: &world.sphere, model, shader: d.shader, ctx, cull: Cull::Back, blend: Blend::Opaque });

    // Halo atmosférico o corona: cáscara más grande, caras traseras, mezcla aditiva.
    let shell = match (body, d.halo) {
        (Body::Helios, _) => Some((ShaderKind::Corona { shell: 1.9 }, 1.9)),
        (_, Some(h)) => Some((ShaderKind::Halo(h), h.shell)),
        _ => None,
    };
    if let Some((shader, scale)) = shell {
        let r = radius * scale;
        passes.additive.push(DrawCall {
            mesh: &world.sphere,
            model: glm::translation(&center) * glm::scaling(&v3(r, r, r)),
            shader,
            ctx: context(center, radius, &glm::Mat4::identity()),
            cull: Cull::Front,
            blend: Blend::Additive,
        });
    }
}

/// Construye las llamadas de dibujo de una vista y devuelve la posición de la luz.
pub fn build(world: &World, view: View, time: f32) -> (Vec<DrawCall<'_>>, Vec3) {
    let mut passes = Passes::default();
    let light = match view {
        View::Body(body) => {
            add_body(&mut passes, world, body, Vec3::zeros(), 1.0, time);
            distant_sun()
        }
        View::System => {
            for body in Body::ALL {
                let d = def(body);
                let theta = d.phase + d.orbit_speed * time;
                let center = v3(theta.cos() * d.orbit, 0.0, -theta.sin() * d.orbit);
                add_body(&mut passes, world, body, center, d.system_radius, time);
            }
            Vec3::zeros() // el sol es la fuente de luz puntual
        }
    };
    let mut draws = passes.opaque;
    draws.extend(passes.alpha);
    draws.extend(passes.additive);
    (draws, light)
}
