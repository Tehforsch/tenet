use bevy::prelude::Component;
use bevy::prelude::Entity;

use crate::communication::Identified;
use crate::units::Length;
use crate::units::VecDimensionless;

#[cfg(feature = "2d")]
pub type FaceArea = crate::units::Length;

#[cfg(not(feature = "2d"))]
pub type FaceArea = crate::units::Area;

#[derive(Clone)]
pub enum Neighbour {
    Local(Entity),
    Remote(RemoteNeighbour),
}

impl Neighbour {
    pub fn local_entity(&self) -> Entity {
        match self {
            Neighbour::Local(entity) => *entity,
            Neighbour::Remote(remote) => remote.local_entity,
        }
    }
}

#[derive(Clone)]
pub struct RemoteNeighbour {
    pub local_entity: Entity,
    pub remote_entity: Identified<Entity>,
}

#[derive(Component)]
pub struct Cell {
    pub neighbours: Vec<(Face, Neighbour)>,
    pub size: Length,
}

impl Cell {
    pub fn iter_faces(&self) -> impl Iterator<Item = &Face> + '_ {
        self.neighbours.iter().map(|(face, _)| face)
    }

    pub fn iter_downwind_faces<'a>(
        &'a self,
        direction: &'a VecDimensionless,
    ) -> impl Iterator<Item = &Face> + 'a {
        self.neighbours
            .iter()
            .map(|(face, _)| face)
            .filter(|face| face.points_downwind(direction))
    }
}

#[derive(Clone)]
pub struct Face {
    pub area: FaceArea,
    pub normal: VecDimensionless,
}

impl Face {
    pub fn points_upwind(&self, dir: &VecDimensionless) -> bool {
        self.normal.dot(*dir).is_negative()
    }

    pub fn points_downwind(&self, dir: &VecDimensionless) -> bool {
        self.normal.dot(*dir).is_positive()
    }
}