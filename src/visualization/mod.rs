pub mod remote;

use bevy::prelude::shape::Circle;
use bevy::prelude::*;
use bevy::sprite::Mesh2dHandle;

use self::remote::RemoteVisualizationMainThreadPlugin;
use self::remote::RemoteVisualizationSideThreadPlugin;
use crate::communication::Rank;
use crate::physics::LocalParticle;
use crate::physics::RemoteParticle;
use crate::position::Position;
use crate::units::f32::meter;
use crate::units::f32::second;

const CIRCLE_SIZE: f32 = 5.0;

const COLORS: &[Color] = &[Color::RED, Color::BLUE, Color::GREEN, Color::YELLOW];

#[derive(StageLabel)]
pub struct VisualizationStage;

pub struct VisualizationPlugin {
    pub main_rank: bool,
}

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_stage_after(
            CoreStage::Update,
            VisualizationStage,
            SystemStage::parallel(),
        );
        if self.main_rank {
            app.add_plugin(RemoteVisualizationMainThreadPlugin)
                .add_startup_system(setup_camera_system)
                .add_system(show_time_system)
                .add_system_to_stage(VisualizationStage, spawn_sprites_system)
                .add_system_to_stage(VisualizationStage, position_to_translation_system);
        } else {
            app.add_plugin(RemoteVisualizationSideThreadPlugin);
        }
    }
}

pub fn spawn_sprites_system(
    mut commands: Commands,
    local_cells: Query<
        (Entity, &Position),
        (
            With<LocalParticle>,
            Without<RemoteParticle>,
            Without<Mesh2dHandle>,
        ),
    >,
    remote_cells: Query<
        (Entity, &Position, &RemoteParticle),
        (Without<LocalParticle>, Without<Mesh2dHandle>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
) {
    for (entity, pos, rank) in local_cells
        .iter()
        .map(|(entity, pos)| (entity, pos, 0))
        .chain(
            remote_cells
                .iter()
                .map(|(entity, pos, rank)| (entity, pos, rank.0)),
        )
    {
        let handle = meshes.add(Mesh::from(Circle::new(CIRCLE_SIZE)));
        let color = COLORS[rank as usize];
        let material = color_materials.add(ColorMaterial { color, ..default() });
        let circle = ColorMesh2dBundle {
            mesh: handle.into(),
            material,
            transform: Transform::from_translation(position_to_translation(pos)),
            ..default()
        };
        commands.entity(entity).insert_bundle(circle);
    }
}

fn position_to_translation(position: &Position) -> Vec3 {
    let camera_zoom = meter(0.01);
    let pos = *(position.0 / camera_zoom).value();
    Vec3::new(pos.x, pos.y, 0.0)
}

pub fn setup_camera_system(mut commands: Commands) {
    commands.spawn_bundle(Camera2dBundle::default());
}

pub fn position_to_translation_system(mut query: Query<(&mut Transform, &Position)>) {
    for (mut transform, position) in query.iter_mut() {
        transform.translation = position_to_translation(position);
    }
}

fn show_time_system(time: Res<crate::physics::Time>) {
    debug!("Time: {:.3} s", time.0.to_value(second));
}
