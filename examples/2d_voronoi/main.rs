#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

mod camera;
mod vis;

use bevy::prelude::*;
use glam::DVec2;
use raxiom::components::Position;
use raxiom::prelude::*;
use raxiom::units::VecLength;
use raxiom::voronoi::DelaunayTriangulation;
use raxiom::voronoi::TetraIndex;
use vis::DrawTriangle;

use crate::camera::setup_camera_system;
use crate::camera::track_mouse_world_position_system;
use crate::camera::MousePosition;

const HIGHLIGHT_LAYER: f32 = -0.1;
const INTERMEDIATE_LAYER: f32 = -0.5;
const LOW_LAYER: f32 = -2.0;

#[derive(Resource)]
struct Colors {
    red: Handle<ColorMaterial>,
    blue: Handle<ColorMaterial>,
    green: Handle<ColorMaterial>,
}

#[derive(Component, Debug)]
struct VisTriangle {
    index: TetraIndex,
}

fn main() {
    let mut app = App::new();
    app.add_startup_system(add_points_system)
        .add_startup_system(setup_camera_system)
        .add_startup_system_to_stage(StartupStage::PostStartup, show_voronoi_system)
        .add_system(highlight_triangle_system)
        .add_system(track_mouse_world_position_system)
        .add_plugins(DefaultPlugins)
        .run();
}

fn add_points_system(mut commands: Commands) {
    let n_x = 3;
    let n_y = 3;
    for i in 0..n_x {
        for j in 0..n_y {
            commands.spawn((
                LocalParticle,
                Position(VecLength::meters(
                    (i as f64 - n_x as f64 / 2.0) * 0.1,
                    (j as f64 - n_y as f64 / 2.0) as f64 * 0.1,
                )),
            ));
        }
    }
}
fn show_voronoi_system(
    mut commands: Commands,
    particles: Particles<&Position>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let colors = Colors {
        blue: materials.add(ColorMaterial::from(Color::BLUE)),
        red: materials.add(ColorMaterial::from(Color::RED)),
        green: materials.add(ColorMaterial::from(Color::GREEN)),
    };
    let triangulation = DelaunayTriangulation::construct(
        &particles
            .into_iter()
            .map(|x| x.value_unchecked())
            .collect::<Vec<_>>(),
    );
    for p in particles.iter() {
        commands.spawn(ColorMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(5.0).into()).into(),
            material: colors.blue.clone(),
            transform: Transform::from_translation(Vec3::new(
                p.x().value_unchecked() as f32,
                p.y().value_unchecked() as f32,
                1.0,
            )),
            ..default()
        });
    }
    for (index, t) in triangulation.tetras.iter() {
        let triangle = DrawTriangle {
            p1: triangulation.points[t.p1],
            p2: triangulation.points[t.p2],
            p3: triangulation.points[t.p3],
        };
        commands
            .spawn(ColorMesh2dBundle {
                mesh: meshes.add(triangle.get_mesh()).into(),
                material: colors.red.clone(),
                ..default()
            })
            .insert(VisTriangle { index });
    }
    commands.insert_resource(triangulation);
    commands.insert_resource(colors);
}

fn highlight_triangle_system(
    mut particles: Query<(&VisTriangle, &mut Handle<ColorMaterial>, &mut Transform)>,
    triangulation: Res<DelaunayTriangulation>,
    colors: Res<Colors>,
    mouse_pos: Res<MousePosition>,
) {
    let index =
        triangulation.find_containing_tetra(DVec2::new(mouse_pos.0.x as f64, mouse_pos.0.y as f64));
    for (triangle, mut color, mut transform) in particles.iter_mut() {
        if Some(triangle.index) == index {
            *color = colors.red.clone();
            transform.translation.z = HIGHLIGHT_LAYER;
        } else {
            *color = colors.blue.clone();
            transform.translation.z = LOW_LAYER;
        };
    }
    if let Some(index) = index {
        let tetra = &triangulation.tetras[index];
        let neighbours = [tetra.f1, tetra.f2, tetra.f3];
        for face in neighbours.iter() {
            let face = &triangulation.faces[*face];
            if let Some(ref opp) = face.opposing {
                assert!(triangulation.tetras.contains(opp.tetra));
            }
            for (triangle, mut color, mut transform) in particles.iter_mut() {
                if Some(triangle.index) == face.opposing.as_ref().map(|opposing| opposing.tetra) {
                    *color = colors.green.clone();
                    transform.translation.z = INTERMEDIATE_LAYER;
                };
            }
        }
    }
}
