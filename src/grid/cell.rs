use bevy::prelude::Component;

use crate::communication::Rank;
use crate::dimension::ActiveWrapType;
use crate::particle::ParticleId;
use crate::units::Length;
use crate::units::VecDimensionless;
use crate::units::Volume;

#[cfg(feature = "2d")]
pub type FaceArea = crate::units::Length;

#[cfg(not(feature = "2d"))]
pub type FaceArea = crate::units::Area;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub enum ParticleType {
    Local(ParticleId),
    Remote(RemoteNeighbour),
    Boundary,
    PeriodicHalo(PeriodicNeighbour),
}

impl ParticleType {
    pub fn is_boundary(&self) -> bool {
        match self {
            Self::Boundary => true,
            Self::Local(_) => false,
            Self::Remote(p) => p.periodic_wrap_type.is_periodic(),
            Self::PeriodicHalo(_) => true,
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            Self::Local(_) => true,
            Self::Boundary => false,
            Self::Remote(_) => false,
            Self::PeriodicHalo(_) => false,
        }
    }

    pub fn unwrap_id(&self) -> ParticleId {
        match self {
            Self::Local(particle_id) => *particle_id,
            Self::Remote(neighbour) => neighbour.id,
            Self::PeriodicHalo(periodic) => periodic.id,
            _ => panic!("Unwrap id called on boundary neighbour"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub struct RemoteNeighbour {
    pub id: ParticleId,
    pub rank: Rank,
    pub periodic_wrap_type: ActiveWrapType,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy)]
pub struct PeriodicNeighbour {
    pub id: ParticleId,
    pub periodic_wrap_type: ActiveWrapType,
}

#[derive(Debug, Component, Clone)]
pub struct Cell {
    pub neighbours: Vec<(Face, ParticleType)>,
    pub size: Length,
    pub volume: Volume,
}

impl Cell {
    pub fn iter_faces(&self) -> impl Iterator<Item = &Face> + '_ {
        self.neighbours.iter().map(|(face, _)| face)
    }

    pub fn volume(&self) -> Volume {
        self.volume
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

#[derive(Clone, Debug)]
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
