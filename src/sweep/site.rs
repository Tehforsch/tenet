use super::count_by_dir::CountByDir;
use super::direction::Directions;
use super::Species;
use crate::chemistry::Chemistry;
use crate::chemistry::Photons;
use crate::units::helpers::Float;
use crate::units::Density;
use crate::units::Time;

#[derive(Debug)]
pub struct Site<C: Chemistry> {
    pub num_missing_upwind: CountByDir,
    pub incoming_total_flux: Vec<C::Photons>,
    pub outgoing_total_flux: Vec<C::Photons>,
    pub species: Species<C>,
    pub density: Density,
    pub change_timescale: Time,
    source: C::Photons,
}

impl<C: Chemistry> Site<C> {
    pub fn new(
        directions: &Directions,
        species: Species<C>,
        density: Density,
        source: C::Photons,
    ) -> Self {
        Self {
            species,
            density,
            source,
            num_missing_upwind: CountByDir::empty(),
            incoming_total_flux: directions.enumerate().map(|_| C::Photons::zero()).collect(),
            outgoing_total_flux: directions.enumerate().map(|_| C::Photons::zero()).collect(),
            change_timescale: Time::zero(),
        }
    }

    pub fn total_incoming_flux(&self) -> C::Photons {
        self.incoming_total_flux.iter().cloned().sum()
    }

    pub fn source_per_direction_bin(&self, directions: &Directions) -> C::Photons {
        self.source.clone() / directions.len() as Float
    }
}
