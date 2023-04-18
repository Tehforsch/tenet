use bevy::prelude::debug;
use bevy::prelude::Commands;
use bevy::prelude::Component;
use bevy::prelude::Res;
use bevy::prelude::Resource;
use derive_custom::Named;
use derive_more::Deref;
use derive_more::DerefMut;
use derive_more::From;
use hdf5::H5Type;
use mpi::traits::Equivalence;
use ordered_float::OrderedFloat;
use raxiom::components;
use raxiom::components::Position;
use raxiom::domain::Decomposition;
use raxiom::domain::IntoKey;
use raxiom::io::input::read_dataset;
use raxiom::io::input::InputFiles;
use raxiom::io::DatasetDescriptor;
use raxiom::io::DatasetShape;
use raxiom::io::InputDatasetDescriptor;
use raxiom::prelude::Communicator;
use raxiom::prelude::Particles;
use raxiom::prelude::SimulationBox;
use raxiom::prelude::WorldRank;
use raxiom::units::Dimensionless;
use raxiom::units::Length;
use raxiom::units::Mass;
use raxiom::units::SourceRate;
use raxiom::units::Time;
use raxiom::units::VecLength;

use crate::cosmology::Cosmology;
use crate::read_vec;
use crate::unit_reader::ArepoUnitReader;

#[derive(Debug, Equivalence, Clone, PartialOrd, PartialEq)]
pub struct DistanceToSourceData(Length);

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "metallicity"]
#[repr(transparent)]
pub struct Metallicity(pub Dimensionless);

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "stellar_formation_time"]
#[repr(transparent)]
// This is dimensionless in the arepo outputs, since its the scale factor
pub struct StellarFormationTime(pub Dimensionless);

#[derive(Clone, Debug, Equivalence)]
pub struct Source {
    position: VecLength,
    age: Time,
    metallicity: Dimensionless,
    mass: Mass,
}

impl Source {
    fn get_source_term(&self) -> SourceRate {
        // Not implemented yet
        SourceRate::new_unchecked(1e55)
    }
}

fn formation_time_to_age(_formation_time: Dimensionless) -> Time {
    // Not implemented yet
    Time::zero()
}

#[derive(Resource)]
pub struct Sources {
    sources: Vec<Source>,
}

fn make_descriptor<T>(
    unit_reader: &ArepoUnitReader,
    name: &str,
    shape: DatasetShape<T>,
) -> InputDatasetDescriptor<T> {
    InputDatasetDescriptor::<T> {
        descriptor: DatasetDescriptor {
            dataset_name: name.into(),
            unit_reader: Box::new(unit_reader.clone()),
        },
        shape,
    }
}

fn read_sources(files: &InputFiles, cosmology: &Cosmology) -> Vec<Source> {
    let unit_reader = ArepoUnitReader::new(cosmology.clone());
    let descriptor = &make_descriptor::<Position>(
        &unit_reader,
        "PartType4/Coordinates",
        DatasetShape::TwoDimensional(read_vec),
    );
    let position = read_dataset(&descriptor, files);
    let descriptor = &make_descriptor::<Metallicity>(
        &unit_reader,
        "PartType4/GFM_Metallicity",
        DatasetShape::OneDimensional,
    );
    let metallicity = read_dataset(&descriptor, files);
    let descriptor = &make_descriptor::<StellarFormationTime>(
        &unit_reader,
        "PartType4/GFM_StellarFormationTime",
        DatasetShape::OneDimensional,
    );
    let formation_time = read_dataset(&descriptor, files);
    let descriptor = &make_descriptor::<components::Mass>(
        &unit_reader,
        "PartType4/Masses",
        DatasetShape::OneDimensional,
    );
    let mass = read_dataset(&descriptor, files);
    position
        .zip(metallicity)
        .zip(formation_time)
        .zip(mass)
        .map(|(((position, metallicity), formation_time), mass)| Source {
            position: *position,
            metallicity: *metallicity,
            mass: *mass,
            age: formation_time_to_age(*formation_time),
        })
        .collect()
}

pub fn read_sources_system(
    mut commands: Commands,
    files: Res<InputFiles>,
    cosmology: Res<Cosmology>,
) {
    let sources = read_sources(&files, &cosmology);
    commands.insert_resource(Sources { sources });
}

pub fn set_source_terms_system(
    mut particles: Particles<(&Position, &mut components::Source)>,
    mut source_comm: Communicator<Source>,
    sources: Res<Sources>,
    decomposition: Res<Decomposition>,
    box_: Res<SimulationBox>,
    world_rank: Res<WorldRank>,
) {
    let all_sources = source_comm.all_gather_varcount(&sources.sources);
    for s in all_sources {
        let key = s.position.into_key(&*box_);
        let rank = decomposition.get_owning_rank(key);
        if rank == **world_rank {
            let closest = particles
                .iter_mut()
                .map(|(pos, source)| {
                    let dist = **pos - s.position;
                    (OrderedFloat(dist.length().value_unchecked()), source)
                })
                .min_by_key(|(dist, _)| *dist);
            let (_, mut source_term) = closest.unwrap();
            **source_term += s.get_source_term();
        }
    }
    let total: SourceRate = particles
        .iter()
        .into_iter()
        .map(|(_, source)| **source)
        .sum();
    debug!("Total luminosity: {:+.2e}", total.in_photons_per_s());
}