#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use std::f64::consts::PI;

use bevy::prelude::*;
use derive_more::From;
use hdf5::H5Type;
use ordered_float::OrderedFloat;
use raxiom::components;
use raxiom::components::IonizedHydrogenFraction;
use raxiom::components::Position;
use raxiom::grid::init_cartesian_grid_system;
use raxiom::io::time_series::TimeSeriesPlugin;
use raxiom::prelude::*;
use raxiom::sweep::initialize_sweep_components_system;
use raxiom::units::Dimensionless;
use raxiom::units::Length;
use raxiom::units::NumberDensity;
use raxiom::units::PhotonFlux;
use raxiom::units::Time;
use raxiom::units::Volume;

#[raxiom_parameters("rtype")]
struct Parameters {
    cell_size: Length,
    number_density: NumberDensity,
    initial_fraction_ionized_hydrogen: Dimensionless,
    source_strength: PhotonFlux,
}

fn main() {
    let mut sim = SimulationBuilder::new();
    let mut sim = sim
        .parameters_from_relative_path(file!(), "parameters.yml")
        .headless(false)
        .write_output(true)
        .read_initial_conditions(false)
        .update_from_command_line_options()
        .build();
    let parameters = sim
        .add_parameter_type_and_get_result::<Parameters>()
        .clone();
    sim.add_startup_system(
        move |commands: Commands,
              box_size: Res<SimulationBox>,
              world_size: Res<WorldSize>,
              world_rank: Res<WorldRank>| {
            init_cartesian_grid_system(
                commands,
                box_size,
                parameters.cell_size,
                world_size,
                world_rank,
            )
        },
    )
    .add_startup_system_to_stage(
        SimulationStartupStages::InsertDerivedComponents,
        initialize_sweep_components_system,
    )
    .add_startup_system_to_stage(
        SimulationStartupStages::InsertDerivedComponents,
        initialize_source_system,
    )
    .add_system_to_stage(SimulationStages::Integration, print_ionization_system)
    .add_plugin(TimeSeriesPlugin::<RTypeRadius>::default())
    .add_plugin(TimeSeriesPlugin::<RTypeError>::default())
    .add_plugin(SweepPlugin)
    .run();
}

fn initialize_source_system(
    mut commands: Commands,
    particles: Particles<(Entity, &Position)>,
    parameters: Res<Parameters>,
    box_size: Res<SimulationBox>,
) {
    let closest_entity_to_center = particles
        .iter()
        .min_by_key(|(_, pos)| {
            let dist = ***pos - box_size.center();
            OrderedFloat(dist.length().value_unchecked())
        })
        .map(|(entity, _)| entity)
        .unwrap();
    commands
        .entity(closest_entity_to_center)
        .insert(components::Source(parameters.source_strength));
}

#[derive(Named, Debug, H5Type, Clone, Deref, From)]
#[name = "rtype_error"]
#[repr(transparent)]
struct RTypeError(Dimensionless);

#[derive(Named, Debug, H5Type, Clone, Deref, From)]
#[name = "rtype_radius"]
#[repr(transparent)]
struct RTypeRadius(Length);

fn print_ionization_system(
    ionization: Query<&IonizedHydrogenFraction>,
    parameters: Res<Parameters>,
    time: Res<raxiom::simulation_plugin::Time>,
    mut radius_writer: EventWriter<RTypeRadius>,
    mut error_writer: EventWriter<RTypeError>,
) {
    let mut volume = Volume::zero();
    for frac in ionization.iter() {
        volume += **frac * parameters.cell_size.powi::<3>();
    }
    let recombination_time = Time::megayears(122.4);
    let stroemgren_radius = Length::kiloparsec(6.79);
    let radius = (volume / (4.0 * PI / 3.0)).cbrt();
    let analytical = (1.0 - (-**time / recombination_time).exp()).cbrt() * stroemgren_radius;
    let error = (radius - analytical).abs() / (radius.max(analytical));
    info!(
        "Radius of ionized region: {:.3?} (analytical: {:.3?}, {:.3?}%)",
        radius,
        analytical,
        error.in_percent(),
    );
    radius_writer.send(RTypeRadius(radius));
    error_writer.send(RTypeError(error));
}
