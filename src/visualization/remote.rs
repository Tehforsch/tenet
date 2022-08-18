use bevy::prelude::*;
use mpi::traits::Equivalence;

use super::VisualizationStage;
use crate::communication::Rank;
use crate::communication::SyncCommunicator;
use crate::particle::RemoteParticleBundle;
use crate::physics::LocalParticle;
use crate::physics::RemoteParticle;
use crate::position::Position;

#[derive(Debug, Equivalence)]
pub struct ParticleVisualizationExchangeData {
    pos: Position,
}

pub(super) struct RemoteVisualizationSideThreadPlugin;

impl Plugin for RemoteVisualizationSideThreadPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(VisualizationStage, send_particles_to_main_thread_system);
    }
}

pub struct RemoteVisualizationMainThreadPlugin;

impl Plugin for RemoteVisualizationMainThreadPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_to_stage(VisualizationStage, receive_particles_on_main_thread_system);
    }
}

fn send_particles_to_main_thread_system(
    rank: Res<Rank>,
    mut communicator: NonSendMut<SyncCommunicator<ParticleVisualizationExchangeData>>,
    particles: Query<(Entity, &Position), With<LocalParticle>>,
) {
    assert!(*rank != 0);
    for (entity, pos) in particles.iter() {
        communicator.send_sync(
            0,
            entity,
            ParticleVisualizationExchangeData { pos: pos.clone() },
        );
    }
    communicator.receive_sync(|_, _| panic!("No items expected"));
}

fn receive_particles_on_main_thread_system(
    mut commands: Commands,
    rank: Res<Rank>,
    mut communicator: NonSendMut<SyncCommunicator<ParticleVisualizationExchangeData>>,
    mut particles: Query<&mut Position, With<RemoteParticle>>,
) {
    assert!(*rank == 0);
    let spawn_particle = |rank: Rank, data: ParticleVisualizationExchangeData| {
        commands
            .spawn()
            .insert_bundle(RemoteParticleBundle::new(data.pos.clone(), rank))
            .id()
    };
    let mut sync = communicator.receive_sync(spawn_particle);
    for (_, entities) in sync.deleted.drain_all() {
        for entity in entities.into_iter() {
            // Decommenting this fixes the bug. wtf
            commands.entity(entity).despawn();
        }
    }
    for (_, data) in sync.updated.drain_all() {
        for (entity, new_pos) in data.into_iter() {
            *particles.get_mut(entity).unwrap() = new_pos.pos;
        }
    }
}