use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use bevy::prelude::App;
use bevy::prelude::Plugin;
use mpi::traits::Equivalence;
use mpi::traits::MatchesRaw;
use mpi::Tag;

use crate::command_line_options::CommandLineOptions;
use crate::communication::local::LocalCommunicator;
use crate::communication::local::Payload;
use crate::communication::plugin::add_communicator;
use crate::communication::plugin::get_next_tag;
use crate::communication::CommunicationPlugin;
use crate::communication::DataByRank;
use crate::communication::NumRanks;
use crate::communication::Rank;
use crate::communication::SizedCommunicator;
use crate::communication::WorldRank;

fn create_and_build_app<
    F: 'static + Sync + Send + Copy + Fn(&mut App, &CommandLineOptions, usize, Rank),
>(
    build_app: F,
    receivers: Receivers,
    senders: Senders,
    opts: &CommandLineOptions,
    rank: Rank,
) -> App {
    let mut app = App::new();
    app.insert_non_send_resource(receivers);
    app.insert_non_send_resource(senders);
    build_app(&mut app, opts, opts.num_threads, rank);
    app
}

pub fn build_local_communication_app<
    F: 'static + Sync + Copy + Send + Fn(&mut App, &CommandLineOptions, usize, Rank),
>(
    build_app: F,
) {
    use clap::Parser;

    let opts = CommandLineOptions::parse();
    let mut app = create_and_build_app(
        build_app,
        Receivers(HashMap::new()),
        Senders(HashMap::new()),
        &opts,
        0,
    );
    for rank in 1..opts.num_threads {
        let receivers = Receivers({
            let all = &mut app
                .world
                .get_non_send_resource_mut::<Receivers>()
                .unwrap()
                .0;
            let to_move = all
                .drain_filter(|comm, _| comm.owner == rank as Rank)
                .collect();
            to_move
        });
        let senders = Senders({
            let all = &mut app.world.get_non_send_resource_mut::<Senders>().unwrap().0;
            let to_move = all
                .drain_filter(|comm, _| comm.owner == rank as Rank)
                .collect();
            to_move
        });
        let opts = opts.clone();
        thread::spawn(move || {
            let mut app = create_and_build_app(build_app, receivers, senders, &opts, rank as Rank);
            app.run()
        });
    }
    app.run();
}

#[derive(PartialEq, Eq, Debug, Hash)]
pub(super) struct Comm {
    owner: Rank,
    other: Rank,
    tag: Tag,
}

pub(super) struct Receivers(HashMap<Comm, Receiver<Payload>>);

struct Senders(HashMap<Comm, Sender<Payload>>);

impl<T> Plugin for CommunicationPlugin<T>
where
    T: Equivalence + Sync + Send + 'static,
    <T as Equivalence>::Out: MatchesRaw,
{
    fn build(&self, app: &mut App) {
        let tag = get_next_tag(app);
        let rank = app.world.get_resource::<WorldRank>().unwrap().0;
        let world_size = app.world.get_resource::<NumRanks>().unwrap().0;
        if rank == 0 {
            let (senders, receivers) = get_senders_and_receivers(world_size, tag);
            app.world
                .get_non_send_resource_mut::<Senders>()
                .unwrap()
                .0
                .extend(senders.into_iter());
            app.world
                .get_non_send_resource_mut::<Receivers>()
                .unwrap()
                .0
                .extend(receivers.into_iter());
        }
        let mut commun = LocalCommunicator::<T>::new(
            DataByRank::empty(),
            DataByRank::empty(),
            tag,
            world_size,
            rank,
        );
        let mut senders = app.world.get_non_send_resource_mut::<Senders>().unwrap();
        add_senders_to_communicator(&mut commun, &mut senders.0);
        let mut receivers = app.world.get_non_send_resource_mut::<Receivers>().unwrap();
        add_receivers_to_communicator(&mut commun, &mut receivers.0);
        add_communicator(self.type_, app, commun);
    }
}

pub(super) fn add_senders_to_communicator<T>(
    communicator: &mut LocalCommunicator<T>,
    senders: &mut HashMap<Comm, Sender<Payload>>,
) {
    for r in communicator.other_ranks() {
        let sender = senders
            .remove(&Comm {
                owner: communicator.rank(),
                other: r,
                tag: communicator.tag(),
            })
            .unwrap();
        communicator.senders.insert(r, sender);
    }
}

pub(super) fn add_receivers_to_communicator<T>(
    communicator: &mut LocalCommunicator<T>,
    receivers: &mut HashMap<Comm, Receiver<Payload>>,
) {
    for r in communicator.other_ranks() {
        let receiver = receivers
            .remove(&Comm {
                owner: communicator.rank(),
                other: r,
                tag: communicator.tag(),
            })
            .unwrap();
        communicator.receivers.insert(r, receiver);
    }
}

pub(super) fn get_senders_and_receivers(
    num_threads: usize,
    tag: Tag,
) -> (
    HashMap<Comm, Sender<Payload>>,
    HashMap<Comm, Receiver<Payload>>,
) {
    let mut senders = HashMap::new();
    let mut receivers = HashMap::new();
    for rank1 in 0i32..num_threads as i32 {
        for rank2 in 0i32..num_threads as i32 {
            if rank1 == rank2 {
                continue;
            }
            let (sender1, receiver1) = channel();
            let (sender2, receiver2) = channel();
            receivers.insert(
                Comm {
                    owner: rank1,
                    other: rank2,
                    tag,
                },
                receiver1,
            );
            receivers.insert(
                Comm {
                    owner: rank2,
                    other: rank1,
                    tag,
                },
                receiver2,
            );
            senders.insert(
                Comm {
                    owner: rank2,
                    other: rank1,
                    tag,
                },
                sender1,
            );
            senders.insert(
                Comm {
                    owner: rank1,
                    other: rank2,
                    tag,
                },
                sender2,
            );
        }
    }
    (senders, receivers)
}