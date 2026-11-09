//! Laboratorio 5 - Gráficas por Computadora (CC2018, UVG).
//! Sistema planetario renderizado con un software renderer y shaders 100 % procedurales.
//!
//! Uso:
//!   cargo run --release                       -> modo interactivo
//!   cargo run --release -- --capturas [vista] -> genera screenshots/*.png (todas o la indicada)
//!   cargo run --release -- --bench            -> mide el tiempo por cuadro de cada vista
//!   cargo run --release -- --generar-esfera   -> regenera assets/sphere.obj

mod camera;
mod framebuffer;
mod mesh;
mod obj;
mod pipeline;
mod png;
mod scene;
mod shaders;

use camera::OrbitCamera;
use framebuffer::Framebuffer;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use pipeline::{BackgroundCache, FrameUniforms};
use scene::{Body, View, World};
use std::path::Path;
use std::time::Instant;

const WIDTH: usize = 960;
const HEIGHT: usize = 720;
/// Factor de supersampling para las capturas generadas con --capturas.
const CAPTURE_SSAA: usize = 2;
/// Presupuesto por cuadro de la resolución dinámica (~30 FPS).
const TARGET_FRAME_MS: f32 = 33.0;
/// Escala mínima de la resolución interna.
const MIN_SCALE: f32 = 0.5;

/// Renderiza una vista completa en el framebuffer.
fn render_view(fb: &mut Framebuffer, bg: &mut BackgroundCache, world: &World, view: View, camera: &OrbitCamera, time: f32) {
    let (draws, light_pos) = scene::build(world, view, time);
    let aspect = fb.width as f32 / fb.height as f32;
    let frame = FrameUniforms {
        view: camera.view(),
        projection: camera.projection(aspect),
        camera_pos: camera.eye(),
        light_pos,
        time,
        basis: camera.basis(),
        tan_half_fov: (camera.fov_y * 0.5).tan(),
    };
    pipeline::render(fb, &draws, &frame, bg);
}

fn save_screenshot(fb: &Framebuffer, view: View, factor: usize) -> std::io::Result<String> {
    let path = format!("screenshots/{}.png", view.file_name());
    let (w, h, rgb) = fb.to_rgb8_downsampled(factor);
    png::write_png(Path::new(&path), w, h, &rgb)?;
    Ok(path)
}

fn all_views() -> impl Iterator<Item = View> {
    Body::ALL.map(View::Body).into_iter().chain([View::System])
}

/// Genera las capturas de todas las vistas sin abrir ventana (con supersampling).
/// `filter` permite generar solo las vistas cuyo nombre de archivo lo contenga.
fn capture_all(world: &World, filter: Option<&str>) {
    let mut fb = Framebuffer::new(WIDTH * CAPTURE_SSAA, HEIGHT * CAPTURE_SSAA);
    let mut bg = BackgroundCache::default();
    for view in all_views().filter(|v| filter.is_none_or(|f| v.file_name().contains(f))) {
        let preset = scene::preset(view);
        let start = Instant::now();
        render_view(&mut fb, &mut bg, world, view, &preset.camera, preset.capture_time);
        match save_screenshot(&fb, view, CAPTURE_SSAA) {
            Ok(path) => println!("{:<26} -> {path} ({:.0} ms)", view.name(), start.elapsed().as_secs_f32() * 1000.0),
            Err(e) => eprintln!("Error al guardar {}: {e}", view.name()),
        }
    }
}

/// Mide el tiempo promedio por cuadro de cada vista a resolución completa.
fn benchmark(world: &World) {
    const FRAMES: u32 = 10;
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let mut bg = BackgroundCache::default();
    println!("Rendimiento a {WIDTH}x{HEIGHT}, resolución completa ({FRAMES} cuadros por vista):");
    for view in all_views() {
        let preset = scene::preset(view);
        render_view(&mut fb, &mut bg, world, view, &preset.camera, preset.capture_time);
        let start = Instant::now();
        for i in 0..FRAMES {
            render_view(&mut fb, &mut bg, world, view, &preset.camera, preset.capture_time + i as f32 * 0.05);
        }
        let ms = start.elapsed().as_secs_f32() * 1000.0 / FRAMES as f32;
        println!("  {:<26} {ms:6.1} ms/cuadro  ({:.1} FPS)", view.name(), 1000.0 / ms);
    }
}

fn print_help() {
    println!("Controles:");
    println!("  1-6        Helios | Terra+Selene | Aurelia | Ignis | Cristalis | Nyx Lumina");
    println!("  0          Sistema completo (órbitas alrededor del sol)");
    println!("  Flechas    Rotar la cámara alrededor del cuerpo");
    println!("  + / -      Acercar / alejar (también rueda del ratón)");
    println!("  P          Pausar / reanudar la animación");
    println!("  R          Activar / desactivar la resolución dinámica");
    println!("  S          Guardar captura a resolución completa en screenshots/<nombre>.png");
    println!("  Esc        Salir");
}

fn run_interactive(world: World) {
    let mut window = match Window::new("Lab 5 - Shaders procedurales", WIDTH, HEIGHT, WindowOptions::default()) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("No se pudo crear la ventana: {e}");
            return;
        }
    };
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let mut bg = BackgroundCache::default();
    let mut buffer = vec![0u32; WIDTH * HEIGHT];

    let mut view = View::System;
    let mut preset = scene::preset(view);
    let mut camera = preset.camera;
    let mut time = 0.0f32;
    let mut paused = false;
    let mut dynamic_resolution = true;
    let mut scale = 1.0f32;
    let mut needs_redraw = true;
    let mut last = Instant::now();
    let (mut fps_timer, mut frames) = (0.0f32, 0u32);

    print_help();
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let dt = last.elapsed().as_secs_f32().min(0.1);
        last = Instant::now();
        if !paused {
            time += dt;
            needs_redraw = true;
        }

        // Teclas de un solo disparo: cambio de vista, pausa, resolución y captura.
        let mut screenshot = false;
        for key in window.get_keys_pressed(KeyRepeat::No) {
            let next = match key {
                Key::Key0 | Key::NumPad0 => Some(View::System),
                Key::Key1 | Key::NumPad1 => Some(View::Body(Body::Helios)),
                Key::Key2 | Key::NumPad2 => Some(View::Body(Body::Terra)),
                Key::Key3 | Key::NumPad3 => Some(View::Body(Body::Aurelia)),
                Key::Key4 | Key::NumPad4 => Some(View::Body(Body::Ignis)),
                Key::Key5 | Key::NumPad5 => Some(View::Body(Body::Cristalis)),
                Key::Key6 | Key::NumPad6 => Some(View::Body(Body::Nyx)),
                Key::P => {
                    paused = !paused;
                    None
                }
                Key::R => {
                    dynamic_resolution = !dynamic_resolution;
                    needs_redraw = true;
                    None
                }
                Key::S => {
                    screenshot = true;
                    None
                }
                _ => None,
            };
            if let Some(v) = next {
                view = v;
                preset = scene::preset(view);
                camera = preset.camera;
                needs_redraw = true;
            }
        }

        // Controles continuos de cámara.
        let before = (camera.yaw, camera.pitch, camera.distance);
        let rot = 1.4 * dt;
        if window.is_key_down(Key::Left) {
            camera.orbit(-rot, 0.0);
        }
        if window.is_key_down(Key::Right) {
            camera.orbit(rot, 0.0);
        }
        if window.is_key_down(Key::Up) {
            camera.orbit(0.0, rot);
        }
        if window.is_key_down(Key::Down) {
            camera.orbit(0.0, -rot);
        }
        if window.is_key_down(Key::Equal) || window.is_key_down(Key::NumPadPlus) {
            camera.zoom(1.0 - 1.2 * dt, preset.min_distance, preset.max_distance);
        }
        if window.is_key_down(Key::Minus) || window.is_key_down(Key::NumPadMinus) {
            camera.zoom(1.0 + 1.2 * dt, preset.min_distance, preset.max_distance);
        }
        if let Some((_, wheel)) = window.get_scroll_wheel() {
            camera.zoom(1.0 - wheel * 0.05, preset.min_distance, preset.max_distance);
        }
        let moving = before != (camera.yaw, camera.pitch, camera.distance);
        needs_redraw |= moving;

        // Resolución dinámica: baja la resolución interna solo mientras hay
        // movimiento; con la escena quieta se dibuja a resolución completa.
        let animating = !paused || moving;
        let target_scale = if dynamic_resolution && animating { scale } else { 1.0 };
        let w = ((WIDTH as f32 * target_scale) as usize).max(2);
        let h = ((HEIGHT as f32 * target_scale) as usize).max(2);
        if fb.width != w || fb.height != h {
            fb = Framebuffer::new(w, h);
            needs_redraw = true;
        }

        if needs_redraw {
            let start = Instant::now();
            render_view(&mut fb, &mut bg, &world, view, &camera, time);
            let ms = start.elapsed().as_secs_f32() * 1000.0;
            if dynamic_resolution && animating {
                let ratio = (TARGET_FRAME_MS / ms.max(1.0)).sqrt().clamp(0.9, 1.08);
                scale = ((scale * ratio).clamp(MIN_SCALE, 1.0) * 20.0).round() / 20.0;
            }
            fb.write_scaled_u32(&mut buffer, WIDTH, HEIGHT);
            needs_redraw = false;
        }

        if screenshot {
            let mut full = Framebuffer::new(WIDTH, HEIGHT);
            render_view(&mut full, &mut BackgroundCache::default(), &world, view, &camera, time);
            match save_screenshot(&full, view, 1) {
                Ok(path) => println!("Captura guardada en {path}"),
                Err(e) => eprintln!("No se pudo guardar la captura: {e}"),
            }
        }

        if window.update_with_buffer(&buffer, WIDTH, HEIGHT).is_err() {
            break;
        }

        frames += 1;
        fps_timer += dt;
        if fps_timer >= 0.5 {
            let fps = frames as f32 / fps_timer;
            let state = if paused { " | pausa" } else { "" };
            let res = if dynamic_resolution { format!("{}%", (fb.width * 100) / WIDTH) } else { "100% fija".into() };
            window.set_title(&format!("Lab 5 - {} | {fps:.0} FPS | resolución {res}{state}", view.name()));
            fps_timer = 0.0;
            frames = 0;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--generar-esfera") {
        let path = Path::new("assets").join("sphere.obj");
        match mesh::write_uv_sphere_obj(&path, 48, 96) {
            Ok(()) => println!("Esfera escrita en {}", path.display()),
            Err(e) => eprintln!("Error al escribir la esfera: {e}"),
        }
        return;
    }

    let world = match World::load() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}\nEjecuta primero: cargo run --release -- --generar-esfera");
            return;
        }
    };
    if let Some(i) = args.iter().position(|a| a == "--capturas") {
        capture_all(&world, args.get(i + 1).map(String::as_str));
    } else if args.iter().any(|a| a == "--bench") {
        benchmark(&world);
    } else {
        run_interactive(world);
    }
}
