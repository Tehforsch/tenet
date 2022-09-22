use std::marker::PhantomData;

use bevy::prelude::Res;
use bevy::prelude::ResMut;

use super::add_output_system;
use super::attribute::Attribute;
use super::OutputFile;
use crate::named::Named;
use crate::simulation::RaxiomPlugin;
use crate::simulation::Simulation;

#[derive(Named)]
pub struct AttributeOutputPlugin<T> {
    _marker: PhantomData<T>,
}

impl<T> Default for AttributeOutputPlugin<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData::default(),
        }
    }
}

impl<T> RaxiomPlugin for AttributeOutputPlugin<T>
where
    T: Attribute + Sync + Send + 'static,
{
    fn build_everywhere(&self, sim: &mut Simulation) {
        add_output_system::<T, _>(sim, Self::write_attribute);
    }
}

impl<T> AttributeOutputPlugin<T>
where
    T: Attribute + Sync + Send + 'static,
{
    fn write_attribute(res: Res<T>, file: ResMut<OutputFile>) {
        let f = file.f.as_ref().unwrap();
        let attr = f
            .new_attr::<T::Output>()
            .shape(())
            .create(T::name())
            .unwrap();
        attr.write_scalar(&res.to_value()).unwrap();
    }
}
