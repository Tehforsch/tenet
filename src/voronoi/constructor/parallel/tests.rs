use bevy::ecs::system::Commands;
use bevy::prelude::Res;

use crate::components::Position;
use crate::domain::DomainPlugin;
use crate::parameters::SimulationBox;
use crate::parameters::SimulationParameters;
use crate::parameters::TreeParameters;
use crate::prelude::Extent;
use crate::prelude::LocalParticle;
use crate::prelude::ThreeD;
use crate::prelude::WorldRank;
use crate::simulation::Simulation;
use crate::simulation_plugin::SimulationStartupStages;
use crate::stages::SimulationStagesPlugin;
use crate::units::Time;
use crate::units::VecLength;
use crate::voronoi::constructor::parallel::plugin::ParallelVoronoiGridConstruction;
use crate::voronoi::test_utils::TestDimension;

#[test]
#[ignore]
fn parallel_voronoi_construction() {
    for num_ranks in 1..10 {
        println!("Running on {}", num_ranks);
        todo!()
        // build_local_communication_sim_with_custom_logic(
        //     |sim: &mut Simulation| build_sim(sim),
        //     |mut sim| {
        //         sim.update();
        //     },
        //     num_ranks,
        // );
    }
}

fn build_sim(sim: &mut Simulation) {
    let box_ = SimulationBox::new(Extent::from_min_max(
        VecLength::meters(0.1, 0.1, 0.1),
        VecLength::meters(0.4, 0.4, 0.4),
    ));
    sim.add_parameter_file_contents("{}".into())
        .add_parameters_explicitly(TreeParameters {
            ..Default::default()
        })
        .add_plugin(SimulationStagesPlugin)
        .add_plugin(ParallelVoronoiGridConstruction)
        .add_required_component::<Position>()
        .add_plugin(DomainPlugin)
        .add_parameters_explicitly(box_)
        .add_parameters_explicitly(SimulationParameters {
            final_time: Some(Time::zero()),
        })
        .add_startup_system_to_stage(
            SimulationStartupStages::InsertComponents,
            spawn_particles_system,
        );
}

fn spawn_particles_system(mut commands: Commands, rank: Res<WorldRank>) {
    for p in ThreeD::get_example_point_set_num(20, **rank as usize) {
        commands.spawn((LocalParticle, Position(VecLength::new_unchecked(p))));
    }
}
