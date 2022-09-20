use bevy::prelude::Component;
use bevy::prelude::Deref;
use bevy::prelude::DerefMut;
use derive_more::From;
use hdf5::H5Type;
use mpi::traits::Equivalence;

use crate::named::Named;
use crate::units::VecVelocity;

#[derive(H5Type, Component, Debug, Clone, Equivalence, Deref, DerefMut, From, Named)]
#[name = "velocity"]
#[repr(C)]
pub struct Velocity(pub VecVelocity);
