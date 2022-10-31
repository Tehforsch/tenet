#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use std::ops::Div;

use bevy::prelude::*;
use hdf5::H5Type;
use mpi::traits::Equivalence;
use rand::Rng;
use raxiom::components;
use raxiom::components::Position;
use raxiom::components::Timestep;
use raxiom::components::Velocity;
use raxiom::prelude::*;
use raxiom::units::Force;
use raxiom::units::Length;
use raxiom::units::Mass;
use raxiom::units::Time;
use raxiom::units::VecLength;
use raxiom::units::VecVelocity;
use serde::Deserialize;

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut)]
#[repr(transparent)]
struct ParticleType(usize);

impl Named for ParticleType {
    fn name() -> &'static str {
        "particle_type"
    }
}

#[derive(Default, Deserialize, Clone)]
struct Parameters {
    num_particles: usize,
    fake_viscosity_timescale: Time,
    box_size: VecLength,
    x_force: Force,
    y_force_factor: <Force as Div<Length>>::Output,
    y_offset: Length,
    particle_mass: Mass,
}

// Implementing named myself here because of
// https://github.com/rust-lang/rust/issues/54363
impl Named for Parameters {
    fn name() -> &'static str {
        "example"
    }
}

fn main() {
    let mut sim = SimulationBuilder::new();
    sim.parameters_from_relative_path(file!(), "parameters.yml")
        .read_initial_conditions(false)
        .write_output(false)
        .headless(false)
        .update_from_command_line_options()
        .build()
        .add_component_no_io::<ParticleType>()
        .add_parameter_type::<Parameters>()
        .add_plugin(HydrodynamicsPlugin)
        .add_startup_system(spawn_particles_system)
        .add_system(external_force_system)
        .add_system(fake_periodic_boundaries_system.after(external_force_system))
        .add_system(fake_viscosity_system.after(external_force_system))
        .run();
}

fn get_y_offset_of_particle_type(parameters: &Parameters, type_: usize) -> Length {
    match type_ {
        0 => parameters.y_offset,
        1 => -parameters.y_offset,
        _ => unreachable!(),
    }
}

fn external_force_system(
    mut particles: Particles<(
        &Position,
        &components::Mass,
        &mut Velocity,
        &ParticleType,
        &Timestep,
    )>,
    parameters: Res<Parameters>,
) {
    for (pos, mass, mut vel, type_, timestep) in particles.iter_mut() {
        let center = VecLength::new_y(get_y_offset_of_particle_type(&parameters, type_.0));
        let mut acceleration = (center - **pos) * parameters.y_force_factor;
        acceleration.set_x(match type_.0 {
            0 => parameters.x_force,
            1 => -parameters.x_force,
            _ => unreachable!(),
        });
        **vel += acceleration / **mass * **timestep;
    }
}

fn fake_viscosity_system(
    mut particles: Particles<(&mut Velocity, &Timestep)>,
    parameters: Res<Parameters>,
) {
    for (mut vel, timestep) in particles.iter_mut() {
        **vel = **vel
            * (-**timestep / parameters.fake_viscosity_timescale)
                .value()
                .exp();
    }
}

fn fake_periodic_boundaries_system(
    mut particles: Particles<&mut Position>,
    parameters: Res<Parameters>,
) {
    for mut pos in particles.iter_mut() {
        if pos.x() > parameters.box_size.x() / 2.0 {
            **pos -= VecLength::new(parameters.box_size.x(), Length::zero());
        } else if pos.x() < -parameters.box_size.x() / 2.0 {
            **pos += VecLength::new(parameters.box_size.x(), Length::zero());
        }
    }
}

fn spawn_particles_system(
    mut commands: Commands,
    rank: Res<WorldRank>,
    parameters: Res<Parameters>,
) {
    if !rank.is_main() {
        return;
    }
    let num_particles_per_type = parameters.num_particles / 2;
    let mut rng = rand::thread_rng();
    for type_ in [0, 1] {
        for _ in 0..num_particles_per_type {
            let offset = get_y_offset_of_particle_type(&parameters, type_);
            let x = rng.gen_range(-parameters.box_size.x()..parameters.box_size.x());
            let y = rng.gen_range(-parameters.box_size.y()..parameters.box_size.y()) + offset;
            spawn_particle(
                &mut commands,
                VecLength::new(x, y),
                VecVelocity::zero(),
                parameters.particle_mass,
                ParticleType(type_),
            )
        }
    }
}

fn spawn_particle(
    commands: &mut Commands,
    pos: VecLength,
    vel: VecVelocity,
    mass: Mass,
    type_: ParticleType,
) {
    commands.spawn_bundle((
        LocalParticle,
        Position(pos),
        Velocity(vel),
        components::Mass(mass),
        type_,
    ));
}
