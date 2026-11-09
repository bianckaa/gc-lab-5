//! Render pipeline por software:
//!
//! 1. Vertex shader: objeto -> mundo -> clip -> pantalla.
//! 2. Ensamblado de primitivas y culling (cara trasera / delantera / ninguno).
//! 3. Rasterización con coordenadas baricéntricas e interpolación con
//!    corrección de perspectiva.
//! 4. Fragment shader (módulo `shaders`) y prueba de profundidad (z-buffer).
//! 5. Mezcla: opaco, alfa o aditivo (halos atmosféricos y corona).
//!
//! La pantalla se divide en franjas horizontales que se rasterizan en
//! paralelo con `rayon`; dentro de cada franja las llamadas de dibujo se
//! procesan en orden, así el resultado es idéntico al secuencial.

use crate::camera::CameraBasis;
use crate::framebuffer::Framebuffer;
use crate::mesh::{Mesh, Vertex};
use crate::shaders::{self, background, FragmentIn, ShadeContext, ShaderKind};
use nalgebra_glm as glm;
use rayon::prelude::*;

/// Altura en píxeles de cada franja de trabajo paralelo.
const BAND_HEIGHT: usize = 8;
/// Vértices con w menor a este valor quedan detrás del plano cercano.
const NEAR_W: f32 = 0.02;
/// Atributos interpolados: posición objeto (3), mundo (3), normal (3), uv (2).
const ATTRS: usize = 11;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cull {
    Back,
    Front,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend {
    /// Reemplaza el color y escribe profundidad.
    Opaque,
    /// Mezcla por alfa; escribe profundidad si el fragmento es casi opaco.
    Alpha,
    /// Suma luz (halos, corona); no escribe profundidad.
    Additive,
}

/// Una llamada de dibujo: malla + transformación + shader propio.
pub struct DrawCall<'a> {
    pub mesh: &'a Mesh,
    pub model: glm::Mat4,
    pub shader: ShaderKind,
    pub ctx: ShadeContext,
    pub cull: Cull,
    pub blend: Blend,
}

/// Datos uniformes del cuadro completo.
pub struct FrameUniforms {
    pub view: glm::Mat4,
    pub projection: glm::Mat4,
    pub camera_pos: glm::Vec3,
    pub light_pos: glm::Vec3,
    pub time: f32,
    pub basis: CameraBasis,
    pub tan_half_fov: f32,
}

#[derive(Clone, Copy)]
struct ScreenVertex {
    x: f32,
    y: f32,
    z: f32,
    inv_w: f32,
    /// Atributos ya divididos entre w (interpolación con perspectiva).
    attrs: [f32; ATTRS],
}

struct Triangle {
    v: [ScreenVertex; 3],
    inv_area: f32,
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    draw: usize,
}

/// Vertex shader: transforma un vértice y prepara sus atributos.
#[inline]
fn vertex_shader(
    vertex: &Vertex,
    model: &glm::Mat4,
    normal_matrix: &glm::Mat3,
    view_proj: &glm::Mat4,
    width: f32,
    height: f32,
) -> Option<ScreenVertex> {
    let p = vertex.position;
    let world = model * glm::vec4(p.x, p.y, p.z, 1.0);
    let clip = view_proj * world;
    if clip.w < NEAR_W {
        return None;
    }
    let inv_w = 1.0 / clip.w;
    let n = normal_matrix * vertex.normal;
    let uv = vertex.uv;
    Some(ScreenVertex {
        x: (clip.x * inv_w * 0.5 + 0.5) * width,
        y: (0.5 - clip.y * inv_w * 0.5) * height,
        z: clip.z * inv_w,
        inv_w,
        attrs: [
            p.x * inv_w, p.y * inv_w, p.z * inv_w,
            world.x * inv_w, world.y * inv_w, world.z * inv_w,
            n.x * inv_w, n.y * inv_w, n.z * inv_w,
            uv.x * inv_w, uv.y * inv_w,
        ],
    })
}

#[inline(always)]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Etapa de vértices + ensamblado de triángulos + culling.
fn assemble(draws: &[DrawCall], frame: &FrameUniforms, width: usize, height: usize) -> Vec<Triangle> {
    let view_proj = frame.projection * frame.view;
    let (w, h) = (width as f32, height as f32);
    let per_draw: Vec<Vec<Triangle>> = draws
        .par_iter()
        .enumerate()
        .map(|(draw_index, draw)| {
            let normal_matrix = glm::transpose(&glm::inverse(&glm::mat4_to_mat3(&draw.model)));
            let screen: Vec<Option<ScreenVertex>> = draw
                .mesh
                .vertices
                .iter()
                .map(|v| vertex_shader(v, &draw.model, &normal_matrix, &view_proj, w, h))
                .collect();
            let mut out = Vec::new();
            for t in draw.mesh.indices.chunks_exact(3) {
                let (Some(a), Some(b), Some(c)) =
                    (screen[t[0] as usize], screen[t[1] as usize], screen[t[2] as usize])
                else {
                    continue;
                };
                let area = edge(a.x, a.y, b.x, b.y, c.x, c.y);
                if area.abs() < 1e-6 {
                    continue;
                }
                // En pantalla (y hacia abajo) las caras frontales quedan con área negativa.
                let front = area < 0.0;
                match draw.cull {
                    Cull::Back if !front => continue,
                    Cull::Front if front => continue,
                    _ => {}
                }
                let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
                let max_x = a.x.max(b.x).max(c.x).ceil().min(w - 1.0) as i32;
                let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
                let max_y = a.y.max(b.y).max(c.y).ceil().min(h - 1.0) as i32;
                if min_x > max_x || min_y > max_y {
                    continue;
                }
                out.push(Triangle { v: [a, b, c], inv_area: 1.0 / area, min_x, max_x, min_y, max_y, draw: draw_index });
            }
            out
        })
        .collect();
    per_draw.into_iter().flatten().collect()
}

/// Caché del fondo estelar: se recalcula solo cuando cambian la cámara o el tamaño.
#[derive(Default)]
pub struct BackgroundCache {
    key: Vec<f32>,
    /// Nebulosa por píxel.
    nebula: Vec<[f32; 3]>,
    /// Estrella por píxel: color (r, g, b) y fase de centelleo.
    stars: Vec<[f32; 4]>,
}

impl BackgroundCache {
    fn ensure(&mut self, width: usize, height: usize, frame: &FrameUniforms) {
        let b = &frame.basis;
        let key = vec![
            width as f32, height as f32, frame.tan_half_fov,
            b.forward.x, b.forward.y, b.forward.z, b.up.x, b.up.y, b.up.z,
        ];
        if key == self.key {
            return;
        }
        self.key = key;
        self.nebula.resize(width * height, [0.0; 3]);
        self.stars.resize(width * height, [0.0; 4]);
        let aspect = width as f32 / height as f32;
        self.nebula
            .par_chunks_mut(width)
            .zip(self.stars.par_chunks_mut(width))
            .enumerate()
            .for_each(|(y, (neb_row, star_row))| {
                let ndc_y = 1.0 - 2.0 * (y as f32 + 0.5) / height as f32;
                for x in 0..width {
                    let ndc_x = 2.0 * (x as f32 + 0.5) / width as f32 - 1.0;
                    let dir = glm::normalize(
                        &(b.forward + b.right * (ndc_x * frame.tan_half_fov * aspect) + b.up * (ndc_y * frame.tan_half_fov)),
                    );
                    let s = background::sample(&dir);
                    neb_row[x] = [s.nebula.x, s.nebula.y, s.nebula.z];
                    star_row[x] = [s.star.x, s.star.y, s.star.z, s.phase];
                }
            });
    }
}

/// Renderiza un cuadro completo: fondo estelar + todas las llamadas de dibujo.
pub fn render(fb: &mut Framebuffer, draws: &[DrawCall], frame: &FrameUniforms, bg: &mut BackgroundCache) {
    let (width, height) = (fb.width, fb.height);
    bg.ensure(width, height, frame);
    let triangles = assemble(draws, frame, width, height);

    // Binning: cada franja recibe la lista ordenada de triángulos que la tocan.
    let band_count = height.div_ceil(BAND_HEIGHT);
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); band_count];
    for (i, t) in triangles.iter().enumerate() {
        for band in (t.min_y as usize / BAND_HEIGHT)..=(t.max_y as usize / BAND_HEIGHT) {
            bins[band].push(i as u32);
        }
    }

    let (nebula, stars) = (&bg.nebula, &bg.stars);
    fb.color
        .par_chunks_mut(width * BAND_HEIGHT)
        .zip(fb.depth.par_chunks_mut(width * BAND_HEIGHT))
        .enumerate()
        .for_each(|(band, (color, depth))| {
            let y0 = band * BAND_HEIGHT;
            let rows = color.len() / width;

            // Fondo procedural (desde la caché, con centelleo animado) y limpieza del z-buffer.
            let offset = y0 * width;
            for i in 0..color.len() {
                let n = nebula[offset + i];
                let s = stars[offset + i];
                color[i] = if s[0] > 0.0 {
                    let k = background::twinkle(s[3], frame.time);
                    [n[0] + s[0] * k, n[1] + s[1] * k, n[2] + s[2] * k]
                } else {
                    n
                };
                depth[i] = f32::INFINITY;
            }

            for &ti in &bins[band] {
                let tri = &triangles[ti as usize];
                rasterize(tri, &draws[tri.draw], frame, color, depth, y0, rows, width);
            }
        });
}

/// Rasteriza un triángulo dentro de una franja y ejecuta el fragment shader.
#[allow(clippy::too_many_arguments)]
#[inline]
fn rasterize(
    tri: &Triangle,
    draw: &DrawCall,
    frame: &FrameUniforms,
    color: &mut [[f32; 3]],
    depth: &mut [f32],
    y0: usize,
    rows: usize,
    width: usize,
) {
    let [a, b, c] = &tri.v;
    let y_start = tri.min_y.max(y0 as i32);
    let y_end = tri.max_y.min((y0 + rows) as i32 - 1);

    for y in y_start..=y_end {
        let py = y as f32 + 0.5;
        let row = (y as usize - y0) * width;
        for x in tri.min_x..=tri.max_x {
            let px = x as f32 + 0.5;
            // Coordenadas baricéntricas normalizadas.
            let w0 = edge(b.x, b.y, c.x, c.y, px, py) * tri.inv_area;
            let w1 = edge(c.x, c.y, a.x, a.y, px, py) * tri.inv_area;
            let w2 = edge(a.x, a.y, b.x, b.y, px, py) * tri.inv_area;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = w0 * a.z + w1 * b.z + w2 * c.z;
            let idx = row + x as usize;
            if z >= depth[idx] || z < -1.0 {
                continue;
            }

            // Interpolación con corrección de perspectiva.
            let inv_w = 1.0 / (w0 * a.inv_w + w1 * b.inv_w + w2 * c.inv_w);
            let mut at = [0.0f32; ATTRS];
            for k in 0..ATTRS {
                at[k] = (w0 * a.attrs[k] + w1 * b.attrs[k] + w2 * c.attrs[k]) * inv_w;
            }
            let pos_world = glm::vec3(at[3], at[4], at[5]);
            let frag = FragmentIn {
                pos_obj: glm::vec3(at[0], at[1], at[2]),
                normal: glm::normalize(&glm::vec3(at[6], at[7], at[8])),
                uv: glm::vec2(at[9], at[10]),
                pos_world,
                time: frame.time,
                light_dir: glm::normalize(&(frame.light_pos - pos_world)),
                view_dir: glm::normalize(&(frame.camera_pos - pos_world)),
                camera_pos: frame.camera_pos,
                ctx: &draw.ctx,
            };

            let Some(out) = shaders::shade(&draw.shader, &frag) else {
                continue; // fragmento descartado (p. ej. huecos de los anillos)
            };
            let dst = &mut color[idx];
            match draw.blend {
                Blend::Opaque => {
                    *dst = [out.x, out.y, out.z];
                    depth[idx] = z;
                }
                Blend::Alpha => {
                    let a = out.w.clamp(0.0, 1.0);
                    for k in 0..3 {
                        dst[k] += (out[k] - dst[k]) * a;
                    }
                    if a > 0.35 {
                        depth[idx] = z;
                    }
                }
                Blend::Additive => {
                    for k in 0..3 {
                        dst[k] += out[k] * out.w;
                    }
                }
            }
        }
    }
}
