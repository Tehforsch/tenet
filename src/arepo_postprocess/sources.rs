use bevy_ecs::prelude::Commands;
use bevy_ecs::prelude::Component;
use bevy_ecs::prelude::Res;
use derive_custom::Named;
use derive_more::Deref;
use derive_more::DerefMut;
use derive_more::From;
use hdf5::H5Type;
use mpi::traits::Equivalence;
use subsweep::components;
use subsweep::components::Position;
use subsweep::cosmology::Cosmology;
use subsweep::impl_to_dataset;
use subsweep::io::input::Reader;
use subsweep::io::DatasetShape;
use subsweep::parameters::InputParameters;
use subsweep::source_systems::Source;
use subsweep::source_systems::Sources;
use subsweep::units;
use subsweep::units::ArepoGarbageUnit;
use subsweep::units::Dimension;
use subsweep::units::Dimensionless;
use subsweep::units::Mass;
use subsweep::units::MassRate;
use subsweep::units::PhotonRate;
use subsweep::units::Time;
use subsweep::units::VecLength;

use super::bpass::bpass_lookup;
use super::unit_reader::make_descriptor;
use super::unit_reader::read_vec;
use super::unit_reader::ArepoGarbageUnitReader;
use super::unit_reader::ArepoUnitReader;
use super::Parameters;

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "metallicity"]
#[repr(transparent)]
pub struct Metallicity(pub Dimensionless);

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "stellar_formation_time"]
#[repr(transparent)]
// This is dimensionless in the arepo outputs, since its the scale factor
pub struct StellarFormationTime(pub Dimensionless);

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "accretion_rate"]
#[repr(transparent)]
pub struct AccretionRate(pub MassRate);

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "mdot"]
#[repr(transparent)]
pub struct MDot(pub ArepoGarbageUnit);

impl_to_dataset!(StellarFormationTime, units::Dimensionless, true);
impl_to_dataset!(Metallicity, units::Dimensionless, true);
impl_to_dataset!(AccretionRate, units::MassRate, true);
impl_to_dataset!(MDot, units::ArepoGarbageUnit, true);

pub fn read_sources_system(
    mut commands: Commands,
    parameters: Res<InputParameters>,
    run_parameters: Res<Parameters>,
    cosmology: Res<Cosmology>,
) {
    let reader = Reader::split_between_ranks(parameters.all_input_files());
    let from_ics = run_parameters.sources.unwrap_from_ics();
    let mut sources = vec![];
    if let Some(escape_frac) = from_ics.escape_fraction {
        sources.extend(read_stellar_sources(&reader, &cosmology, escape_frac));
    }
    if let Some(escape_frac) = from_ics.escape_fraction_agn {
        sources.extend(read_agn_sources(&reader, &cosmology, escape_frac));
    }
    commands.insert_resource(Sources { sources });
}

fn new_bpass_source(
    cosmology: &Cosmology,
    position: VecLength,
    metallicity: Dimensionless,
    mass: Mass,
    formation_scale_factor: Dimensionless,
    escape_fraction: Dimensionless,
) -> Source {
    let age = formation_scale_factor_to_age(cosmology, formation_scale_factor);
    Source {
        pos: position,
        rate: bpass_lookup(age, metallicity, mass) * escape_fraction,
    }
}

fn formation_scale_factor_to_age(
    cosmology: &Cosmology,
    formation_scale_factor: Dimensionless,
) -> Time {
    cosmology.time_difference_between_scalefactors(formation_scale_factor, cosmology.scale_factor())
}

fn read_stellar_sources(
    reader: &Reader,
    cosmology: &Cosmology,
    escape_fraction: Dimensionless,
) -> Vec<Source> {
    let unit_reader = ArepoUnitReader::new(cosmology.clone());
    let descriptor = make_descriptor::<Position, _>(
        &unit_reader,
        "PartType4/Coordinates",
        DatasetShape::TwoDimensional(read_vec),
    );
    let position = reader.read_dataset(descriptor);
    let descriptor = make_descriptor::<Metallicity, _>(
        &unit_reader,
        "PartType4/GFM_Metallicity",
        DatasetShape::OneDimensional,
    );
    let metallicity = reader.read_dataset(descriptor);
    let descriptor = make_descriptor::<StellarFormationTime, _>(
        &unit_reader,
        "PartType4/GFM_StellarFormationTime",
        DatasetShape::OneDimensional,
    );
    let formation_scale_factor = reader.read_dataset(descriptor);
    let descriptor = make_descriptor::<components::Mass, _>(
        &unit_reader,
        "PartType4/Masses",
        DatasetShape::OneDimensional,
    );
    let mass = reader.read_dataset(descriptor);
    position
        .zip(metallicity)
        .zip(formation_scale_factor)
        .zip(mass)
        // Everything else is WIND. Love the data structures in Arepo
        .filter(|(((_, _), formation_scale_factor), _)| formation_scale_factor.is_positive())
        .map(
            |(((position, metallicity), formation_scale_factor), mass)| {
                new_bpass_source(
                    cosmology,
                    *position,
                    *metallicity,
                    *mass,
                    *formation_scale_factor,
                    escape_fraction,
                )
            },
        )
        .collect()
}

fn read_agn_sources(
    reader: &Reader,
    cosmology: &Cosmology,
    escape_fraction: Dimensionless,
) -> Vec<Source> {
    let unit_reader = ArepoUnitReader::new(cosmology.clone());
    let descriptor = make_descriptor::<Position, _>(
        &unit_reader,
        "PartType5/Coordinates",
        DatasetShape::TwoDimensional(read_vec),
    );
    let position = reader.read_dataset(descriptor);
    let garbage_unit_reader = ArepoGarbageUnitReader(Dimension {
        mass: -1,
        length: 2,
        time: -1,
        ..Dimension::none()
    });
    let descriptor = make_descriptor::<MDot, _>(
        &garbage_unit_reader,
        "PartType5/BH_Mdot",
        DatasetShape::OneDimensional,
    );
    let accretion_rate = reader.read_dataset(descriptor).into_iter().map(|acc| {
        AccretionRate(acc.value_unchecked() * Mass::solar(1e10) / Time::gigayears(0.978))
    });
    position
        .zip(accretion_rate)
        .map(|(position, accretion_rate)| {
            new_agn_source(*position, accretion_rate, escape_fraction)
        })
        .collect()
}

fn new_agn_source(
    position: VecLength,
    _accretion_rate: AccretionRate,
    escape_fraction: Dimensionless,
) -> Source {
    Source {
        pos: position,
        // Obtained by integrating the spectrum from Feltre et al 2016
        // from 13.6eV to infinity
        rate: PhotonRate::photons_per_second(4.8e54) * escape_fraction,
    }
}
