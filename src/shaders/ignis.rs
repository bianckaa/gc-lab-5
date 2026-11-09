//! EXTRA "Ignis": planeta volcánico con corteza de basalto y lava emisiva.

use super::utils::*;
use super::{opaque, FragmentIn};

/// Color de la lava según su temperatura (0 = enfriándose, 1 = incandescente).
const LAVA: [Stop; 4] = [
    (0.00, [0.35, 0.03, 0.01]),
    (0.35, [0.95, 0.20, 0.02]),
    (0.75, [1.00, 0.55, 0.08]),
    (1.00, [1.00, 0.92, 0.55]),
];

pub fn shade(f: &FragmentIn) -> Option<Vec4> {
    let n = f.n_obj();
    let t = f.time;

    // CAPA 1: corteza de basalto oscuro con relieve rugoso (fbm + ridged).
    let rock = fbm(&(n * 5.0), 4);
    let ridges = ridged(&(n * 9.0 + v3(4.0, 1.0, 7.0)), 4);
    let mut crust = mix3(&rgb(0x1f1916), &rgb(0x5a4a40), smoothstep(-0.2, 0.25, rock));
    crust = mix3(&crust, &rgb(0x4a2a1c), smoothstep(0.1, 0.3, fbm(&(n * 2.0 + v3(5.0, 5.0, 5.0)), 3)) * 0.5);
    crust *= 0.6 + 0.7 * ridges;

    // CAPA 2: grietas de lava por bordes de voronoi (F2 - F1), emisivas y pulsantes.
    let cracks_domain = warp(&(n * 5.5), 0.7, 2);
    let plates = voronoi(&cracks_domain);
    // Ancho e intensidad variables: algunas grietas están activas y otras casi frías.
    let activity = smoothstep(-0.15, 0.2, fbm(&(n * 3.0 + v3(2.0, 9.0, 4.0)), 3));
    let width = 0.03 + 0.09 * activity;
    let crack = smoothstep(width, 0.0, plates.edge());
    let crack_core = smoothstep(width * 0.35, 0.0, plates.edge());
    let pulse = 0.7 + 0.3 * (t * 2.2 + plates.id * 25.0).sin();
    let heat = (crack * 0.5 + crack_core * 0.5) * (0.35 + 0.65 * activity);
    let mut emission = ramp(&LAVA, heat) * crack * pulse * (0.4 + 1.4 * activity);
    // Las placas se oscurecen junto a las grietas (borde enfriado).
    crust *= 0.55 + 0.45 * smoothstep(0.0, 0.2, plates.edge());

    // CAPA 3: ríos de lava (fbm ridged sobre dominio deformado) y calderas brillantes.
    let river_field = fbm(&warp(&(n * 2.3 + v3(1.0, 8.0, 3.0)), 0.6, 2), 4);
    let river = smoothstep(0.035, 0.0, river_field.abs()) * smoothstep(-0.1, 0.15, rock);
    let flow = 0.75 + 0.25 * noise(&(n * 30.0 + v3(0.0, -t * 0.6, t * 0.4)));
    emission += ramp(&LAVA, 0.5 + 0.4 * flow) * river * flow * 1.5;
    let calderas = voronoi(&(n * 2.2 + v3(9.0, 2.0, 5.0)));
    if calderas.id > 0.72 {
        let size = 0.16 + 0.10 * fract(calderas.id * 17.0);
        let d = calderas.f1 / size;
        let breathe = 0.8 + 0.2 * (t * 1.4 + calderas.id * 40.0).sin();
        let pool = smoothstep(1.0, 0.75, d);
        let rim = (-((d - 1.05) / 0.12).powi(2)).exp();
        let swirl = fbm(&(n * 18.0 + v3(t * 0.2, 0.0, 0.0)), 3);
        // Costra flotante: placas oscuras sobre el lago de lava.
        let floating = voronoi(&(n * 26.0 + v3(t * 0.05, 0.0, 0.0)));
        let crusted = smoothstep(0.12, 0.2, floating.edge()) * smoothstep(0.3, 0.85, d);
        crust = mix3(&crust, &rgb(0x090606), (rim * 0.8).min(1.0));
        let glow = ramp(&LAVA, 0.55 + 0.45 * swirl) * pool * breathe * 1.8;
        emission += glow * (1.0 - crusted * 0.85);
        crust = mix3(&crust, &rgb(0x120c0a), pool * crusted);
    }

    // Iluminación de la corteza (la lava no necesita luz: es emisiva).
    let shadow = f.external_shadow();
    let diffuse = soft_diffuse(&f.normal, &f.light_dir, 0.1) * shadow;
    let mut color = crust * (0.03 + 0.95 * diffuse) + emission;

    // CAPA 4: ceniza y humo atmosférico rojizo que deriva y se tiñe con la lava.
    let smoke_domain = rotate_y(&n, t * 0.025);
    let smoke = smoothstep(0.0, 0.3, fbm(&warp(&(smoke_domain * 2.6), 0.7, 2), 4));
    let glow_below = (emission.x + emission.y) * 0.25;
    let smoke_color = rgb(0x3b2520) * (0.08 + 0.9 * diffuse) + rgb(0xff5a1a) * glow_below.min(0.6) * 0.5;
    color = mix3(&color, &smoke_color, smoke * 0.72);
    let haze = fresnel(&f.normal, &f.view_dir, 2.5);
    color += rgb(0xd2401a) * haze * (0.25 + 0.6 * diffuse);

    opaque(color)
}
