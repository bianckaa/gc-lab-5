//! Cargador mínimo de archivos Wavefront OBJ (solo geometría: v, vt, vn, f).
//! Ignora materiales (.mtl) a propósito: el color sale exclusivamente de los shaders.

use crate::mesh::{Mesh, Vertex};
use nalgebra_glm as glm;
use std::collections::HashMap;
use std::path::Path;

/// Resuelve un índice OBJ (base 1, admite índices negativos relativos).
fn resolve(index: &str, len: usize) -> Option<usize> {
    let i: i64 = index.parse().ok()?;
    match i {
        i if i > 0 => Some(i as usize - 1),
        i if i < 0 => (len as i64 + i).try_into().ok(),
        _ => None,
    }
}

pub fn load_obj(path: &Path) -> Result<Mesh, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("No se pudo leer {}: {e}", path.display()))?;

    let mut positions: Vec<glm::Vec3> = Vec::new();
    let mut uvs: Vec<glm::Vec2> = Vec::new();
    let mut normals: Vec<glm::Vec3> = Vec::new();
    let mut mesh = Mesh::default();
    let mut lookup: HashMap<(usize, Option<usize>, Option<usize>), u32> = HashMap::new();
    let mut missing_normals = false;

    let parse_floats = |parts: &mut std::str::SplitWhitespace, n: usize| -> Vec<f32> {
        parts.take(n).filter_map(|s| s.parse().ok()).collect()
    };

    for (line_no, line) in text.lines().enumerate() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let f = parse_floats(&mut parts, 3);
                if f.len() == 3 {
                    positions.push(glm::vec3(f[0], f[1], f[2]));
                }
            }
            Some("vt") => {
                let f = parse_floats(&mut parts, 2);
                uvs.push(glm::vec2(*f.first().unwrap_or(&0.0), *f.get(1).unwrap_or(&0.0)));
            }
            Some("vn") => {
                let f = parse_floats(&mut parts, 3);
                if f.len() == 3 {
                    normals.push(glm::normalize(&glm::vec3(f[0], f[1], f[2])));
                }
            }
            Some("f") => {
                let mut corners = Vec::new();
                for token in parts {
                    let mut it = token.split('/');
                    let vi = it
                        .next()
                        .and_then(|s| resolve(s, positions.len()))
                        .ok_or_else(|| format!("Índice inválido en la línea {}", line_no + 1))?;
                    let ti = it.next().filter(|s| !s.is_empty()).and_then(|s| resolve(s, uvs.len()));
                    let ni = it.next().filter(|s| !s.is_empty()).and_then(|s| resolve(s, normals.len()));
                    missing_normals |= ni.is_none();
                    let key = (vi, ti, ni);
                    let index = *lookup.entry(key).or_insert_with(|| {
                        mesh.vertices.push(Vertex {
                            position: positions[vi],
                            normal: ni.map(|i| normals[i]).unwrap_or_else(glm::Vec3::zeros),
                            uv: ti.map(|i| uvs[i]).unwrap_or_else(glm::Vec2::zeros),
                        });
                        (mesh.vertices.len() - 1) as u32
                    });
                    corners.push(index);
                }
                // Triangulación en abanico para polígonos de más de 3 vértices.
                for k in 1..corners.len().saturating_sub(1) {
                    mesh.indices.extend_from_slice(&[corners[0], corners[k], corners[k + 1]]);
                }
            }
            _ => {}
        }
    }

    // Si el archivo no trae normales, se promedian las normales de cara.
    if missing_normals {
        let mut acc = vec![glm::Vec3::zeros(); mesh.vertices.len()];
        for t in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.vertices[i as usize].position);
            let n = glm::cross(&(b - a), &(c - a));
            for &i in t {
                acc[i as usize] += n;
            }
        }
        for (v, n) in mesh.vertices.iter_mut().zip(acc) {
            if v.normal == glm::Vec3::zeros() && n.norm() > 0.0 {
                v.normal = glm::normalize(&n);
            }
        }
    }

    if mesh.indices.is_empty() {
        return Err(format!("{} no contiene caras", path.display()));
    }
    Ok(mesh)
}
