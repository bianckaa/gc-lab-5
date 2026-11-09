//! Escritor PNG mínimo para guardar capturas de pantalla.
//! Solo escribe (nunca carga) imágenes; la compresión usa deflate de `flate2`.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

fn write_chunk(out: &mut impl Write, kind: &[u8; 4], data: &[u8]) -> io::Result<()> {
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    out.write_all(&crc.finalize().to_be_bytes())
}

/// Guarda una imagen RGB de 8 bits por canal como PNG.
pub fn write_png(path: &Path, width: usize, height: usize, rgb: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }

    // Cada fila lleva un byte de filtro; se usa el filtro "Sub" (1), que
    // mejora la compresión de degradados suaves.
    let stride = width * 3;
    let mut raw = Vec::with_capacity((stride + 1) * height);
    for row in rgb.chunks_exact(stride) {
        raw.push(1);
        for i in 0..stride {
            let left = if i >= 3 { row[i - 3] } else { 0 };
            raw.push(row[i].wrapping_sub(left));
        }
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw)?;
    let compressed = encoder.finish()?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits, RGB, sin entrelazado

    let mut file = io::BufWriter::new(File::create(path)?);
    file.write_all(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])?;
    write_chunk(&mut file, b"IHDR", &header)?;
    write_chunk(&mut file, b"IDAT", &compressed)?;
    write_chunk(&mut file, b"IEND", &[])?;
    file.flush()
}
