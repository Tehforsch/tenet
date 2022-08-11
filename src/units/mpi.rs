use glam::Vec2;
use mpi::datatype::DatatypeRef;
use mpi::datatype::SystemDatatype;
use mpi::datatype::UserDatatype;
use mpi::ffi;
use mpi::traits::Equivalence;
use mpi::traits::FromRaw;
use once_cell::sync::Lazy;

use super::dimension::Dimension;
use super::quantity::Quantity;

unsafe impl<const D: Dimension> Equivalence for Quantity<f32, D> {
    type Out = SystemDatatype;

    fn equivalent_datatype() -> Self::Out {
        unsafe { DatatypeRef::from_raw(ffi::RSMPI_FLOAT) }
    }
}

unsafe impl<const D: Dimension> Equivalence for Quantity<f64, D> {
    type Out = SystemDatatype;

    fn equivalent_datatype() -> Self::Out {
        unsafe { DatatypeRef::from_raw(ffi::RSMPI_DOUBLE) }
    }
}

unsafe impl<const D: Dimension> Equivalence for Quantity<Vec2, D> {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        static DATATYPE: Lazy<::mpi::datatype::UserDatatype> =
            Lazy::new(|| UserDatatype::contiguous(2, &f32::equivalent_datatype()));
        DATATYPE.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use mpi::traits::Communicator;

    use crate::units::f32::meter;

    #[test]
    fn pack_unpack_quantity() {
        let q1 = meter(1.0);
        let mut q2 = meter(2.0);

        let universe = mpi::initialize().unwrap();
        let world = universe.world();
        let a = world.pack(&q1);
        unsafe {
            world.unpack_into(&a, &mut q2, 0);
        }
    }
}
