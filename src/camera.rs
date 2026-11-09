//! Cámara orbital: gira alrededor de un objetivo con yaw/pitch y distancia.

use nalgebra_glm as glm;

#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub target: glm::Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub fov_y: f32,
}

/// Base ortonormal de la cámara, usada para el fondo estelar.
#[derive(Clone, Copy, Debug)]
pub struct CameraBasis {
    pub forward: glm::Vec3,
    pub right: glm::Vec3,
    pub up: glm::Vec3,
}

impl OrbitCamera {
    pub fn new(yaw: f32, pitch: f32, distance: f32) -> Self {
        Self { target: glm::Vec3::zeros(), yaw, pitch, distance, fov_y: 45f32.to_radians() }
    }

    pub fn eye(&self) -> glm::Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        self.target + glm::vec3(cp * sy, sp, cp * cy) * self.distance
    }

    pub fn view(&self) -> glm::Mat4 {
        glm::look_at(&self.eye(), &self.target, &glm::vec3(0.0, 1.0, 0.0))
    }

    pub fn projection(&self, aspect: f32) -> glm::Mat4 {
        glm::perspective(aspect, self.fov_y, 0.05, 500.0)
    }

    pub fn basis(&self) -> CameraBasis {
        let forward = glm::normalize(&(self.target - self.eye()));
        let right = glm::normalize(&glm::cross(&forward, &glm::vec3(0.0, 1.0, 0.0)));
        let up = glm::cross(&right, &forward);
        CameraBasis { forward, right, up }
    }

    pub fn orbit(&mut self, d_yaw: f32, d_pitch: f32) {
        self.yaw += d_yaw;
        self.pitch = (self.pitch + d_pitch).clamp(-1.45, 1.45);
    }

    pub fn zoom(&mut self, factor: f32, min: f32, max: f32) {
        self.distance = (self.distance * factor).clamp(min, max);
    }
}
