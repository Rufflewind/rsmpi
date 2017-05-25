//! Bridge between rust types and raw values

use std::ops::{Deref, DerefMut};

/// Rust C bridge traits
pub mod traits {
    pub use super::{AsRaw, AsRawMut};
}

/// A rust type than can identify as a raw value understood by the MPI C API.
pub unsafe trait AsRaw {
    /// The raw MPI C API type
    type Raw;
    /// The raw value
    fn as_raw(&self) -> Self::Raw;
}

unsafe impl<T> AsRaw for T where T: Deref, T::Target: AsRaw
{
    type Raw = <T::Target as AsRaw>::Raw;
    fn as_raw(&self) -> Self::Raw {
        (**self).as_raw()
    }
}

/// A rust type than can provide a mutable pointer to a raw value understood by the MPI C API.
pub unsafe trait AsRawMut: AsRaw {
    /// A mutable pointer to the raw value
    fn as_raw_mut(&mut self) -> *mut <Self as AsRaw>::Raw;
}

unsafe impl<T> AsRawMut for T where T: DerefMut, T::Target: AsRawMut
{
    fn as_raw_mut(&mut self) -> *mut <Self as AsRaw>::Raw {
        (**self).as_raw_mut()
    }
}
