#[cfg(test)]
mod tests;

use std::path::PathBuf;

use bevy::prelude::debug;
use bevy::prelude::info;
use bevy::prelude::warn;
use bevy::prelude::Commands;
use bevy::prelude::Component;
use bevy::prelude::Deref;
use bevy::prelude::DerefMut;
use bevy::prelude::Entity;
use bevy::prelude::IntoSystemDescriptor;
use bevy::prelude::NonSend;
use bevy::prelude::Res;
use bevy::prelude::ResMut;
use bevy::prelude::Resource;
use bevy::prelude::SystemLabel;
use derive_custom::raxiom_parameters;
use hdf5::File;
use ndarray::ArrayBase;
use ndarray::Dim;
use ndarray::OwnedRepr;

use super::to_dataset::ToDataset;
use super::InputDatasetDescriptor;
use crate::communication::WorldRank;
use crate::communication::WorldSize;
use crate::hash_map::HashMap;
use crate::io::DatasetShape;
use crate::prelude::Float;
use crate::prelude::LocalParticle;
use crate::prelude::Named;
use crate::simulation::RaxiomPlugin;
use crate::simulation::Simulation;

/// Determines how a component is input into the simulation.
pub enum ComponentInput<T> {
    /// The component needs to be present in the given dataset when the initial conditions are read.
    Required(InputDatasetDescriptor<T>),
    /// The component does not need to be present and will be inserted
    /// by a startup system.
    Derived,
}

#[derive(Default, Deref, DerefMut, Resource)]
pub struct InputFiles(Vec<File>);

/// Parameters describing how the initial conditions
/// should be read. Only required if should_read_initial_conditions
/// is set in the [SimulationBuilder](crate::prelude::SimulationBuilder)
#[derive(Default)]
#[raxiom_parameters("input")]
pub struct InputParameters {
    /// The files containing the initial conditions
    pub paths: Vec<PathBuf>,
    /// Utility for debugging: "Shrink" the ICS by only using every
    /// nth particle.
    pub shrink_factor: Option<usize>,
}

#[derive(Default, Deref, DerefMut, Resource)]
struct SpawnedEntities(Vec<Entity>);

#[derive(Named)]
pub struct DatasetInputPlugin<T> {
    descriptor: InputDatasetDescriptor<T>,
}

impl<T> DatasetInputPlugin<T> {
    pub fn from_descriptor(descriptor: InputDatasetDescriptor<T>) -> Self {
        Self { descriptor }
    }
}

#[derive(SystemLabel)]
struct ReadDatasetLabel;

#[derive(Default, Deref, DerefMut, Resource)]
pub struct RegisteredDatasets(HashMap<String, RegisteredDataset>);

#[derive(Default, Resource)]
pub struct RegisteredDataset {
    name: String,
}

impl<T: Named + ToDataset + Component + Sync + Send + 'static> RaxiomPlugin
    for DatasetInputPlugin<T>
{
    fn allow_adding_twice(&self) -> bool {
        true
    }

    fn should_build(&self, sim: &Simulation) -> bool {
        sim.read_initial_conditions
    }

    fn build_once_everywhere(&self, sim: &mut Simulation) {
        sim.add_parameter_type::<InputParameters>()
            .insert_resource(InputFiles::default())
            .insert_resource(SpawnedEntities::default())
            .add_startup_system(open_file_system)
            .add_startup_system(
                spawn_entities_system
                    .after(open_file_system)
                    .before(close_file_system),
            )
            .add_startup_system(close_file_system.after(spawn_entities_system));
    }

    fn build_everywhere(&self, sim: &mut Simulation) {
        let mut registered_datasets = sim.get_resource_or_insert_with(RegisteredDatasets::default);
        registered_datasets.insert(
            T::name().into(),
            RegisteredDataset {
                name: self.descriptor.dataset_name().into(),
            },
        );
        sim.insert_non_send_resource(self.descriptor.clone())
            .add_startup_system(
                read_dataset_system::<T>
                    .after(open_file_system)
                    .after(spawn_entities_system)
                    .before(close_file_system)
                    .label(ReadDatasetLabel)
                    .ambiguous_with(ReadDatasetLabel),
            );
    }
}

pub fn open_file_system(
    mut files: ResMut<InputFiles>,
    parameters: Res<InputParameters>,
    rank: Res<WorldRank>,
    size: Res<WorldSize>,
) {
    let files_this_rank_should_open: Vec<_> = parameters
        .paths
        .iter()
        .enumerate()
        .filter(|(i, _)| i.rem_euclid(**size) == **rank as usize)
        .map(|(_, file)| file)
        .collect();
    assert!(files.is_empty());
    for path in files_this_rank_should_open.iter() {
        info!(
            "Reading initial conditions file: {}",
            path.to_str().unwrap()
        );
        files.push(File::open(path).unwrap_or_else(|_| {
            panic!(
                "Failed to open initial conditions file: {}",
                path.to_str().unwrap()
            )
        }));
    }
}

pub fn close_file_system(mut files: ResMut<InputFiles>) {
    files.0.clear();
}

fn warn_if_shrink_factor_is_enabled(parameters: &InputParameters) {
    if let Some(shrink_factor) = parameters.shrink_factor {
        if shrink_factor != 1 {
            warn!(
                "Shrinking ICS by using only every {}-th particle",
                shrink_factor
            );
        }
    }
}

fn spawn_entities_system(
    mut commands: Commands,
    mut spawned_entities: ResMut<SpawnedEntities>,
    datasets: Res<RegisteredDatasets>,
    files: Res<InputFiles>,
    parameters: Res<InputParameters>,
) {
    if datasets.len() == 0 {
        return;
    }
    warn_if_shrink_factor_is_enabled(&parameters);
    let (_, example_dataset) = &datasets.iter().next().unwrap();
    let get_num_entities = |dataset_name: &str| {
        files
            .iter()
            .map(|f| f.dataset(dataset_name).unwrap().shape()[0])
            .sum::<usize>()
            / parameters.shrink_factor.unwrap_or(1)
    };
    let num_entities = get_num_entities(&example_dataset.name);
    for (_, dataset) in datasets.iter() {
        let num_entities_this_dataset = get_num_entities(&dataset.name);
        if num_entities_this_dataset != num_entities {
            panic!(
                "Different lengths of datasets: {} ({num_entities}) and {} ({num_entities_this_dataset})", &example_dataset.name, &dataset.name
            );
        }
    }
    debug!("Spawned {} new entities", num_entities);
    assert_eq!(spawned_entities.len(), 0);
    spawned_entities.0 = (0..num_entities)
        .map(|_| commands.spawn((LocalParticle,)).id())
        .collect();
}

fn read_dataset_system<T: ToDataset + Component>(
    descriptor: NonSend<InputDatasetDescriptor<T>>,
    mut commands: Commands,
    files: Res<InputFiles>,
    spawned_entities: Res<SpawnedEntities>,
    parameters: Res<InputParameters>,
) {
    let name = descriptor.dataset_name();
    debug!("Reading dataset {}", name);
    let should_insert = |i: usize| {
        if let Some(shrink_factor) = parameters.shrink_factor {
            i.rem_euclid(shrink_factor) == 0
        } else {
            true
        }
    };
    for (item, entity) in read_dataset::<T>(&descriptor, &files)
        .enumerate()
        .filter(|(i, _)| should_insert(*i))
        .map(|(_, t)| t)
        .zip(spawned_entities.iter())
    {
        commands.entity(*entity).insert(item);
    }
    debug!("Finished reading dataset {}", name);
}

pub fn read_dataset<'a, T: ToDataset + Component>(
    descriptor: &'a InputDatasetDescriptor<T>,
    files: &'a InputFiles,
) -> impl Iterator<Item = T> + 'a {
    let factor_read = T::dimension().base_conversion_factor();
    files.iter().flat_map(move |file| {
        let (set, factor_written) = read_dataset_for_file(descriptor, file);
        set.into_iter()
            .map(move |item| item.convert_base_units(factor_written / factor_read))
    })
}

pub fn read_dataset_for_file<'a, T: ToDataset + Component>(
    descriptor: &'a InputDatasetDescriptor<T>,
    file: &'a File,
) -> (ArrayBase<OwnedRepr<T>, Dim<[usize; 1]>>, f64) {
    let name = descriptor.dataset_name();
    let set = file
        .dataset(&name)
        .unwrap_or_else(|e| panic!("Failed to open dataset: {name}, {e:?}"));
    let data = match descriptor.shape {
        DatasetShape::OneDimensional => set
            .read_1d::<T>()
            .unwrap_or_else(|e| panic!("Failed to read dataset: {name}, {e:?}")),
        DatasetShape::TwoDimensional(constructor) => {
            let d = set
                .read_2d::<Float>()
                .unwrap_or_else(|e| panic!("Failed to read dataset: {name}, {e:?}"));
            d.outer_iter()
                .map(|row| constructor(row.as_slice().unwrap()))
                .collect()
        }
    };
    let conversion_factor = descriptor.read_scale_factor(&set);
    assert_eq!(
        descriptor.read_dimension(&set),
        T::dimension(),
        "Mismatch in dimension while reading dataset {name}.",
    );
    (data, conversion_factor)
}
