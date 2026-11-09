//! Framebuffer de color en punto flotante (HDR) y z-buffer.

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    /// Color lineal RGB por píxel; puede superar 1.0 (sol, lava, auroras).
    pub color: Vec<[f32; 3]>,
    /// Profundidad en NDC por píxel (menor = más cerca).
    pub depth: Vec<f32>,
}

/// Mapeo tonal con hombro suave: conserva los colores de diseño por debajo
/// de 0.8 y comprime con suavidad los valores emisivos intensos.
#[inline]
pub fn tonemap(c: f32) -> f32 {
    const KNEE: f32 = 0.8;
    if c <= KNEE {
        c.max(0.0)
    } else {
        let range = 1.0 - KNEE;
        KNEE + range * (1.0 - (-(c - KNEE) / range).exp())
    }
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![[0.0; 3]; width * height],
            depth: vec![f32::INFINITY; width * height],
        }
    }

    /// Convierte el buffer HDR al formato 0RGB de 32 bits que espera la ventana.
    pub fn write_u32(&self, out: &mut [u32]) {
        for (dst, c) in out.iter_mut().zip(&self.color) {
            let r = (tonemap(c[0]) * 255.0 + 0.5) as u32;
            let g = (tonemap(c[1]) * 255.0 + 0.5) as u32;
            let b = (tonemap(c[2]) * 255.0 + 0.5) as u32;
            *dst = (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
        }
    }

    /// Escala el buffer (posiblemente a menor resolución) al tamaño de la
    /// ventana con interpolación bilineal (resolución dinámica).
    pub fn write_scaled_u32(&self, out: &mut [u32], out_w: usize, out_h: usize) {
        if self.width == out_w && self.height == out_h {
            return self.write_u32(out);
        }
        let sx = self.width as f32 / out_w as f32;
        let sy = self.height as f32 / out_h as f32;
        for (y, row) in out.chunks_exact_mut(out_w).enumerate() {
            let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (self.height - 1) as f32);
            let y0 = fy as usize;
            let y1 = (y0 + 1).min(self.height - 1);
            let ty = fy - y0 as f32;
            for (x, dst) in row.iter_mut().enumerate() {
                let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (self.width - 1) as f32);
                let x0 = fx as usize;
                let x1 = (x0 + 1).min(self.width - 1);
                let tx = fx - x0 as f32;
                let (a, b) = (self.color[y0 * self.width + x0], self.color[y0 * self.width + x1]);
                let (c, d) = (self.color[y1 * self.width + x0], self.color[y1 * self.width + x1]);
                let mut px = 0u32;
                for k in 0..3 {
                    let top = a[k] + (b[k] - a[k]) * tx;
                    let bottom = c[k] + (d[k] - c[k]) * tx;
                    let v = (tonemap(top + (bottom - top) * ty) * 255.0 + 0.5) as u32;
                    px = (px << 8) | v.min(255);
                }
                *dst = px;
            }
        }
    }

    /// Devuelve RGB de 8 bits, promediando bloques `factor x factor`
    /// (supersampling para capturas más nítidas).
    pub fn to_rgb8_downsampled(&self, factor: usize) -> (usize, usize, Vec<u8>) {
        let w = self.width / factor;
        let h = self.height / factor;
        let mut out = Vec::with_capacity(w * h * 3);
        let inv = 1.0 / (factor * factor) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 3];
                for sy in 0..factor {
                    for sx in 0..factor {
                        let c = self.color[(y * factor + sy) * self.width + x * factor + sx];
                        for k in 0..3 {
                            acc[k] += tonemap(c[k]);
                        }
                    }
                }
                for value in acc {
                    out.push((value * inv * 255.0 + 0.5).min(255.0) as u8);
                }
            }
        }
        (w, h, out)
    }
}
