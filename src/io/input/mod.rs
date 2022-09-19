use std::marker::PhantomData;
use std::path::PathBuf;

use bevy::prelude::*;
use hdf5::File;
use serde::Deserialize;

use super::output::dataset_plugin::SCALE_FACTOR_IDENTIFIER;
use super::to_dataset::ToDataset;
use crate::communication::WorldRank;
use crate::communication::WorldSize;
use crate::initial_conditions;
use crate::named::Named;
use crate::physics::LocalParticle;
use crate::plugin_utils::get_parameters;
use crate::plugin_utils::run_once;

#[derive(Default, Deref, DerefMut)]
struct InputFiles(Vec<File>);

#[derive(AmbiguitySetLabel)]
struct InputSystemsAmbiguitySet;

struct InputMarker;

impl Named for InputMarker {
    fn name() -> &'static str {
        "input"
    }
}

#[derive(Clone, Default, Deserialize)]
pub struct Parameters {
    pub initial_condition_paths: Vec<PathBuf>,
}

#[derive(Default, Deref, DerefMut)]
struct SpawnedEntities(Vec<Entity>);

pub struct DatasetInputPlugin<T> {
    _marker: PhantomData<T>,
}

impl<T> Default for DatasetInputPlugin<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData::default(),
        }
    }
}

#[derive(Default, Deref, DerefMut)]
pub struct RegisteredDatasets(Vec<&'static str>);

impl<T: ToDataset + Component + Sync + Send + 'static> Plugin for DatasetInputPlugin<T> {
    fn build(&self, app: &mut App) {
        let parameters = get_parameters::<initial_conditions::Parameters>(app);
        if !matches!(parameters, initial_conditions::Parameters::Read(_)) {
            return;
        }
        run_once::<InputMarker>(app, |app| {
            app.insert_resource(parameters.unwrap_read().clone())
                .insert_resource(InputFiles::default())
                .insert_resource(SpawnedEntities::default())
                .add_startup_system(open_file_system)
                .add_startup_system(
                    spawn_entities_system
                        .after(open_file_system)
                        .before(close_file_system),
                )
                .add_startup_system(close_file_system.after(spawn_entities_system));
        });
        let mut registered_datasets = app
            .world
            .get_resource_or_insert_with(|| RegisteredDatasets::default());
        registered_datasets.push(T::name());
        app.add_startup_system(
            read_dataset_system::<T>
                .after(open_file_system)
                .after(spawn_entities_system)
                .before(close_file_system)
                .in_ambiguity_set(InputSystemsAmbiguitySet),
        );
    }
}

fn open_file_system(
    mut files: ResMut<InputFiles>,
    parameters: Res<Parameters>,
    rank: Res<WorldRank>,
    size: Res<WorldSize>,
) {
    let files_this_rank_should_open: Vec<_> = parameters
        .initial_condition_paths
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
        files.push(File::open(path).expect(&format!(
            "Failed to open initial conditions file: {}",
            path.to_str().unwrap()
        )));
    }
}

fn close_file_system(mut files: ResMut<InputFiles>) {
    files.0.clear();
}

fn spawn_entities_system(
    mut commands: Commands,
    mut spawned_entities: ResMut<SpawnedEntities>,
    datasets: Res<RegisteredDatasets>,
    files: Res<InputFiles>,
) {
    if datasets.len() == 0 {
        return;
    }
    let example_dataset = datasets[0];
    let get_num_entities = |dataset_name: &str| {
        files
            .iter()
            .map(|f| f.dataset(dataset_name).unwrap().shape()[0])
            .sum::<usize>()
    };
    let num_entities = get_num_entities(example_dataset);
    for dataset in datasets.iter() {
        let num_entities_this_dataset = get_num_entities(dataset);
        if num_entities_this_dataset != num_entities {
            panic!(
                "Different lengths of datasets: {} ({}) and {} ({})",
                example_dataset, num_entities, dataset, num_entities_this_dataset
            );
        }
    }
    assert_eq!(spawned_entities.len(), 0);
    spawned_entities.0 = (0..num_entities)
        .map(|_| commands.spawn_bundle((LocalParticle,)).id())
        .collect();
}

fn read_dataset_system<T: ToDataset + Component>(
    mut commands: Commands,
    files: Res<InputFiles>,
    spawned_entities: Res<SpawnedEntities>,
) {
    let name = T::name();
    let data = files.iter().map(|file| {
        let set = file
            .dataset(name)
            .expect(&format!("Failed to open dataset: {}", name));
        let data = set
            .read_1d::<T>()
            .expect(&format!("Failed to read dataset: {}", name));
        let conversion_factor: f64 = set
            .attr(SCALE_FACTOR_IDENTIFIER)
            .expect("No scale factor in dataset")
            .read_scalar()
            .unwrap();
        (data, conversion_factor)
    });
    for ((item, factor_written), entity) in data
        .flat_map(|(set, factor_written)| set.into_iter().map(move |item| (item, factor_written)))
        .zip(spawned_entities.iter())
    {
        let factor_read = T::dimension().base_conversion_factor();
        commands
            .entity(*entity)
            .insert(item.convert_base_units(factor_written / factor_read));
    }
}