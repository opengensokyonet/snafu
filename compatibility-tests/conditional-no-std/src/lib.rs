#![no_std]
#![cfg_attr(
    feature = "unstable-provider-api",
    feature(error_generic_member_access)
)]

use snafu::{IntoError, Snafu};

pub struct Payload<'ctx>(pub &'ctx str);

#[derive(Debug, Snafu)]
#[snafu(display("operation failed"))]
pub struct Failure<E> {
    pub source: E,
}

pub fn construct(text: &str) -> Failure<Payload<'_>> {
    FailureSnafu.into_error(Payload(text))
}

#[derive(Debug, Snafu)]
pub struct Leaf;

pub fn source_chain() -> Failure<Leaf> {
    let error = FailureSnafu.into_error(Leaf);
    let _: &dyn snafu::Error = &error;
    error
}

#[cfg(feature = "futures")]
pub async fn future_context(text: &str) -> Result<(), Failure<Payload<'_>>> {
    use snafu::futures::TryFutureExt;
    core::future::ready(Err(Payload(text)))
        .context(FailureSnafu)
        .await
}

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
mod recursive {
    use super::*;
    use alloc::boxed::Box;

    #[derive(Debug, Snafu)]
    #[snafu(error_compat_bounds(), display_bounds(T: core::fmt::Display))]
    pub enum Recursive<T> {
        #[snafu(display("leaf: {value}"))]
        Leaf { value: T },
        #[snafu(display("nested: {source}"))]
        Nested { source: Box<Recursive<T>> },
    }

    pub fn source_chain() -> Recursive<u8> {
        let leaf: Recursive<u8> = LeafSnafu { value: 42u8 }.build();
        let error = NestedSnafu.into_error(Box::new(leaf));
        let _: &dyn snafu::Error = &error;
        error
    }
}

#[cfg(feature = "alloc")]
pub use recursive::{source_chain as recursive_source_chain, Recursive};
