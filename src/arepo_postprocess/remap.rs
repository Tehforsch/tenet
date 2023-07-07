use std::path::PathBuf;

use bevy::prelude::debug;
use bevy::prelude::info;
use bevy::prelude::Entity;
use bevy::prelude::Res;
use kiddo::distance::squared_euclidean;
use kiddo::KdTree;
use mpi::traits::Equivalence;
use raxiom::communication::communicator::Communicator;
use raxiom::communication::DataByRank;
use raxiom::communication::ExchangeCommunicator;
use raxiom::communication::Identified;
use raxiom::components::IonizedHydrogenFraction;
use raxiom::components::Position;
use raxiom::components::Temperature;
use raxiom::cosmology::LittleH;
use raxiom::cosmology::ScaleFactor;
use raxiom::hash_map::HashMap;
use raxiom::io::input::attribute::read_attribute;
use raxiom::io::input::get_file_or_all_hdf5_files_in_path_if_dir;
use raxiom::io::input::Reader;
use raxiom::io::DatasetShape;
use raxiom::io::DefaultUnitReader;
use raxiom::parameters::Cosmology;
use raxiom::prelude::Float;
use raxiom::prelude::Particles;
use raxiom::units::Dimension;
use raxiom::units::Dimensionless;
use raxiom::units::Length;
use raxiom::units::VecLength;

use super::unit_reader::make_descriptor;
use super::Parameters;

type Tree = KdTree<Float, 3>;

#[derive(Equivalence, Clone, Debug)]
struct SearchRequest {
    pos: Position,
}

#[derive(Equivalence, Clone, Debug)]
struct SearchReply {
    squared_distance: f64,
    data: RemapData,
}

#[derive(Equivalence, Clone, Debug)]
struct RemapData {
    temperature: Temperature,
    ionized_hydrogen_fraction: IonizedHydrogenFraction,
}

fn read_remap_data(
    files: Vec<PathBuf>,
) -> (
    Vec<Position>,
    Vec<IonizedHydrogenFraction>,
    Vec<Temperature>,
    Cosmology,
) {
    let reader = Reader::split_between_ranks(files.iter());
    let unit_reader = DefaultUnitReader;
    let descriptor =
        make_descriptor::<Position, _>(&unit_reader, "position", DatasetShape::OneDimensional);
    let position = reader.read_dataset(descriptor).collect();
    let descriptor = make_descriptor::<IonizedHydrogenFraction, _>(
        &unit_reader,
        "ionized_hydrogen_fraction",
        DatasetShape::OneDimensional,
    );
    let ionized_hydrogen_fraction = reader.read_dataset(descriptor).collect();
    let descriptor = make_descriptor::<Temperature, _>(
        &unit_reader,
        "temperature",
        DatasetShape::OneDimensional,
    );
    let temperature = reader.read_dataset(descriptor).collect();
    let scale_factor = read_attribute::<ScaleFactor>(&files[0]);
    let little_h = read_attribute::<LittleH>(&files[0]);
    let cosmology = Cosmology::Cosmological {
        a: *scale_factor.0,
        h: *little_h.0,
    };
    (position, ionized_hydrogen_fraction, temperature, cosmology)
}

pub fn remap_abundances_and_energies_system(
    parameters: Res<Parameters>,
    cosmology: Res<Cosmology>,
    mut particles: Particles<(
        Entity,
        &Position,
        &mut Temperature,
        &mut IonizedHydrogenFraction,
    )>,
) {
    const CHUNK_SIZE: usize = 50000;
    let files = match &parameters.remap_from {
        Some(file) => get_file_or_all_hdf5_files_in_path_if_dir(file),
        None => return,
    };
    info!("Remapping abundances and temperatures.");

    let (position, ionized_hydrogen_fraction, temperature, remap_cosmology) =
        read_remap_data(files);
    let factor = get_scale_factor_difference(Length::dimension(), &cosmology, &remap_cosmology);
    let requests: Vec<_> = particles
        .iter()
        .map(|(entity, pos, _, _)| Identified::new(entity, SearchRequest { pos: pos.clone() }))
        .collect();
    let num_chunks = global_num_chunks(requests.len(), CHUNK_SIZE);
    let mut chunk_iter = requests.chunks(CHUNK_SIZE);
    let tree: Tree = (&position
        .iter()
        .map(|pos| pos_to_tree_coord(&(**pos * factor)))
        .collect::<Vec<_>>())
        .into();
    for _ in 0..num_chunks {
        let chunk = chunk_iter.next().unwrap_or(&[]);
        exchange_request_chunk(
            &mut particles,
            &ionized_hydrogen_fraction,
            &temperature,
            &tree,
            chunk,
        );
    }
    debug!("Finished remapping.");
}

fn get_scale_factor_difference(
    dimension: Dimension,
    cosmology: &Cosmology,
    remap_cosmology: &Cosmology,
) -> Dimensionless {
    match cosmology {
        Cosmology::Cosmological { .. } => {
            if let Cosmology::Cosmological { .. } = remap_cosmology {
                (*(cosmology.scale_factor() / remap_cosmology.scale_factor()))
                    .powi(dimension.length)
                    .into()
            } else {
                panic!()
            }
        }
        Cosmology::NonCosmological => {
            if let Cosmology::Cosmological { .. } = remap_cosmology {
                panic!()
            } else {
                1.0.into()
            }
        }
    }
}

fn get_reply(
    request: &SearchRequest,
    tree: &Tree,
    temperature: &[Temperature],
    ionized_hydrogen_fraction: &[IonizedHydrogenFraction],
) -> SearchReply {
    let tree_coord = pos_to_tree_coord(&request.pos);
    let (squared_distance, index) = tree.nearest_one(&tree_coord, &squared_euclidean);
    SearchReply {
        squared_distance,
        data: RemapData {
            temperature: temperature[index].clone(),
            ionized_hydrogen_fraction: ionized_hydrogen_fraction[index].clone(),
        },
    }
}

fn exchange_request_chunk(
    particles: &mut Particles<(
        Entity,
        &Position,
        &mut Temperature,
        &mut IonizedHydrogenFraction,
    )>,
    ionized_hydrogen_fraction: &[IonizedHydrogenFraction],
    temperature: &[Temperature],
    tree: &Tree,
    chunk: &[Identified<SearchRequest>],
) {
    let mut comm = ExchangeCommunicator::<Identified<SearchRequest>>::new();
    let mut closest_map: HashMap<_, _> = chunk
        .iter()
        .map(|request| {
            (
                request.entity(),
                get_reply(&request.data, tree, temperature, ionized_hydrogen_fraction),
            )
        })
        .collect();
    let outgoing =
        DataByRank::same_for_other_ranks_in_communicator(chunk.iter().cloned().collect(), &comm);
    let incoming = comm.exchange_all(outgoing);
    let mut outgoing: DataByRank<Vec<Identified<SearchReply>>> =
        DataByRank::from_communicator(&comm);
    for (rank, requests) in incoming {
        for request in requests {
            let reply = Identified::new(
                request.entity(),
                get_reply(&request.data, tree, temperature, ionized_hydrogen_fraction),
            );
            outgoing[rank].push(reply);
        }
    }
    let mut comm = ExchangeCommunicator::<Identified<SearchReply>>::new();
    let incoming = comm.exchange_all(outgoing);
    for (_, replies) in incoming {
        for reply in replies {
            let entity = reply.entity();
            let previously_closest = &closest_map[&entity];
            if previously_closest.squared_distance > reply.data.squared_distance {
                closest_map.insert(entity, reply.data);
            }
        }
    }
    for (entity, closest) in closest_map.into_iter() {
        let (_, _, mut temp, mut ion_frac) = particles.get_mut(entity).unwrap();
        remap_from(&mut temp, &mut ion_frac, closest.data);
    }
}

fn remap_from(temp: &mut Temperature, ion_frac: &mut IonizedHydrogenFraction, data: RemapData) {
    **temp = (*temp).max(*data.temperature);
    **ion_frac = (*ion_frac).max(*data.ionized_hydrogen_fraction);
}

fn pos_to_tree_coord(pos: &VecLength) -> [f64; 3] {
    [
        pos.x().value_unchecked(),
        pos.y().value_unchecked(),
        pos.z().value_unchecked(),
    ]
}

fn global_num_chunks(num_elements: usize, chunk_size: usize) -> usize {
    let mut comm: Communicator<usize> = Communicator::new();
    let num_chunks = div_ceil(num_elements, chunk_size);
    comm.all_gather_max(&num_chunks).unwrap()
}

fn div_ceil(x: usize, y: usize) -> usize {
    (x / y) + if x.rem_euclid(y) > 0 { 1 } else { 0 }
}
