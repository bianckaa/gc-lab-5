//! Mallas y generadores de geometría procedural.
//!
//! - `write_uv_sphere_obj`: genera `assets/sphere.obj` (esfera base del planeta).
//! - `icosphere`: malla propia de la luna Selene (modelo separado del planeta).
//! - `annulus`: anillo plano para los sistemas de anillos (modelo separado).

use nalgebra_glm as glm;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: glm::Vec3,
    pub normal: glm::Vec3,
    pub uv: glm::Vec2,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    /// Ordena cada triángulo en sentido antihorario visto desde afuera
    /// (válido para mallas convexas centradas en el origen).
    fn orient_outward(&mut self) {
        for tri in self.indices.chunks_exact_mut(3) {
            let a = self.vertices[tri[0] as usize].position;
            let b = self.vertices[tri[1] as usize].position;
            let c = self.vertices[tri[2] as usize].position;
            let n = glm::cross(&(b - a), &(c - a));
            if glm::dot(&n, &(a + b + c)) < 0.0 {
                tri.swap(1, 2);
            }
        }
    }
}

/// Coordenadas UV esféricas (se usan solo como dato numérico, nunca como textura).
fn spherical_uv(p: &glm::Vec3) -> glm::Vec2 {
    glm::vec2(0.5 + p.z.atan2(p.x) / TAU, 0.5 - p.y.clamp(-1.0, 1.0).asin() / PI)
}

/// Escribe una esfera UV de radio 1 en formato OBJ (v / vt / vn / f).
pub fn write_uv_sphere_obj(path: &Path, stacks: u32, slices: u32) -> io::Result<()> {
    let mut mesh = Mesh::default();
    for i in 0..=stacks {
        let theta = PI * i as f32 / stacks as f32;
        for j in 0..=slices {
            let phi = TAU * j as f32 / slices as f32;
            let p = glm::vec3(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
            mesh.vertices.push(Vertex {
                position: p,
                normal: p,
                uv: glm::vec2(j as f32 / slices as f32, i as f32 / stacks as f32),
            });
        }
    }
    let row = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = i * row + j;
            let b = (i + 1) * row + j;
            let c = (i + 1) * row + j + 1;
            let d = i * row + j + 1;
            if i != stacks - 1 {
                mesh.indices.extend_from_slice(&[a, b, c]);
            }
            if i != 0 {
                mesh.indices.extend_from_slice(&[a, c, d]);
            }
        }
    }
    mesh.orient_outward();

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "# sphere.obj - esfera UV de radio 1 ({stacks} x {slices})")?;
    writeln!(out, "# Generada por `cargo run --release -- --generar-esfera`")?;
    writeln!(out, "o Sphere")?;
    for v in &mesh.vertices {
        writeln!(out, "v {:.6} {:.6} {:.6}", v.position.x, v.position.y, v.position.z)?;
    }
    for v in &mesh.vertices {
        writeln!(out, "vt {:.6} {:.6}", v.uv.x, v.uv.y)?;
    }
    for v in &mesh.vertices {
        writeln!(out, "vn {:.6} {:.6} {:.6}", v.normal.x, v.normal.y, v.normal.z)?;
    }
    writeln!(out, "s 1")?;
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] + 1, t[1] + 1, t[2] + 1);
        writeln!(out, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}")?;
    }
    out.flush()
}

/// Icosfera de radio 1 generada en código (modelo propio de la luna).
pub fn icosphere(subdivisions: u32) -> Mesh {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut positions: Vec<glm::Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| glm::normalize(&glm::vec3(x, y, z)))
    .collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..subdivisions {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, positions: &mut Vec<glm::Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                let m = glm::normalize(&(positions[a as usize] + positions[b as usize]));
                positions.push(m);
                (positions.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = midpoint(a, b, &mut positions);
            let bc = midpoint(b, c, &mut positions);
            let ca = midpoint(c, a, &mut positions);
            next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        faces = next;
    }
    let mut mesh = Mesh {
        vertices: positions
            .iter()
            .map(|p| Vertex { position: *p, normal: *p, uv: spherical_uv(p) })
            .collect(),
        indices: faces.into_iter().flatten().collect(),
    };
    mesh.orient_outward();
    mesh
}

/// Anillo plano (annulus) en el plano XZ, de doble cara.
/// UV: u = posición radial normalizada, v = ángulo normalizado.
pub fn annulus(inner: f32, outer: f32, segments: u32, rings: u32) -> Mesh {
    let mut mesh = Mesh::default();
    for s in 0..=segments {
        let angle = TAU * s as f32 / segments as f32;
        let (sin, cos) = angle.sin_cos();
        for r in 0..=rings {
            let k = r as f32 / rings as f32;
            let radius = inner + (outer - inner) * k;
            mesh.vertices.push(Vertex {
                position: glm::vec3(radius * cos, 0.0, radius * sin),
                normal: glm::vec3(0.0, 1.0, 0.0),
                uv: glm::vec2(k, s as f32 / segments as f32),
            });
        }
    }
    let row = rings + 1;
    for s in 0..segments {
        for r in 0..rings {
            let a = s * row + r;
            let b = a + 1;
            let c = a + row + 1;
            let d = a + row;
            mesh.indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    mesh
}
