pub mod hydrogen_only;

use std::fmt::Debug;
use std::iter::Sum;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Div;
use std::ops::Mul;
use std::ops::Sub;

use mpi::traits::Equivalence;

use crate::grid::Cell;
use crate::sweep::site::Site;
use crate::units::helpers::Float;
use crate::units::Dimensionless;
use crate::units::Length;
use crate::units::PhotonFlux;
use crate::units::Time;
use crate::units::Volume;

pub trait Chemistry: Sized + 'static {
    type Photons: Photons;
    type Species: Debug;

    fn get_outgoing_flux(
        &self,
        cell: &Cell,
        site: &mut Site<Self>,
        incoming_flux: Self::Photons,
    ) -> Self::Photons;

    fn update_abundances(
        &self,
        site: &mut Site<Self>,
        flux: Self::Photons,
        timestep: Time,
        volume: Volume,
        length: Length,
    ) -> Time;
}

pub trait Photons:
    Sum<Self>
    + Add<Self, Output = Self>
    + AddAssign<Self>
    + Sub<Self, Output = Self>
    + Mul<Float, Output = Self>
    + Mul<Dimensionless, Output = Self>
    + Div<Float, Output = Self>
    + Debug
    + Clone
    + Equivalence
{
    fn zero() -> Self;
}

impl Photons for PhotonFlux {
    fn zero() -> Self {
        PhotonFlux::zero()
    }
}