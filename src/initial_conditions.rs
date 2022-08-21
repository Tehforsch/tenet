use bevy::prelude::Commands;
use bevy::prelude::Res;
use bevy::prelude::*;
use mpi::Rank;
use rand::Rng;

use crate::mass::Mass;
use crate::particle::LocalParticleBundle;
use crate::position::Position;
use crate::units::vec2;
use crate::velocity::Velocity;

pub struct InitialConditionsPlugin;

impl Plugin for InitialConditionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(spawn_particles_system);
    }
}

fn spawn_particles_system(mut commands: Commands, rank: Res<Rank>) {
    if *rank != 0 {
        return;
    }
    let n_particles = 150;
    for _ in 0..n_particles {
        let x = rand::thread_rng().gen_range(-5.0..-4.0);
        let y = rand::thread_rng().gen_range(-1.0..1.0);
        let pos = vec2::Length::meter(Vec2::new(x, y));
        let x = 0.0;
        let y = 0.1;
        let vel = vec2::Velocity::meters_per_second(Vec2::new(x, y)) * 1.0;
        commands.spawn().insert_bundle(LocalParticleBundle::new(
            Position(pos),
            Velocity(vel),
            Mass(crate::units::f32::Mass::kilogram(10000000.0)),
        ));
    }

    for _ in 0..n_particles {
        let x = rand::thread_rng().gen_range(4.0..5.0);
        let y = rand::thread_rng().gen_range(-1.0..1.0);
        let pos = vec2::Length::meter(Vec2::new(x, y));
        let x = 0.0;
        let y = -0.1;
        let vel = vec2::Velocity::meters_per_second(Vec2::new(x, y)) * 1.0;
        commands.spawn().insert_bundle(LocalParticleBundle::new(
            Position(pos),
            Velocity(vel),
            Mass(crate::units::f32::Mass::kilogram(10000000.0)),
        ));
    }
}
