#![cfg(test)]

mod core_functionality {
    use std::io;

    fn io_failure() -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Other, "arbitrary failure"))
    }

    fn implements_error<T: std::error::Error>() {}

    mod enum_style {
        use super::*;
        use snafu::prelude::*;

        #[derive(Debug, Snafu)]
        enum Error {
            #[snafu(display("Without a source: {id}"))]
            WithoutSource { id: i32 },

            #[snafu(display("With a source: {id}"))]
            WithSource { id: i32, source: io::Error },
        }

        type Result<T, E = Error> = std::result::Result<T, E>;

        fn create_without_source() -> Result<()> {
            WithoutSourceSnafu { id: 42 }.fail()
        }

        fn create_with_source() -> Result<()> {
            io_failure().context(WithSourceSnafu { id: 42 })
        }

        #[test]
        fn it_works() {
            implements_error::<Error>();
            let _ = create_without_source();
            let _ = create_with_source();
        }
    }

    mod struct_style {
        use super::*;
        use snafu::prelude::*;

        #[derive(Debug, Snafu)]
        #[snafu(display("Without a source: {id}"))]
        struct WithoutSource {
            id: i32,
        }

        #[derive(Debug, Snafu)]
        #[snafu(display("With a source: {id}"))]
        struct WithSource {
            id: i32,
            source: io::Error,
        }

        fn create_without_source() -> Result<(), WithoutSource> {
            WithoutSourceSnafu { id: 42 }.fail()
        }

        fn create_with_source() -> Result<(), WithSource> {
            io_failure().context(WithSourceSnafu { id: 42 })
        }

        #[test]
        fn it_works() {
            implements_error::<WithoutSource>();
            implements_error::<WithSource>();
            let _ = create_without_source();
            let _ = create_with_source();
        }
    }

    mod opaque_style {
        use super::*;
        use snafu::prelude::*;

        #[derive(Debug, Snafu)]
        struct Dummy;

        #[derive(Debug, Snafu)]
        struct Opaque(Dummy);

        fn create() -> Result<(), Opaque> {
            Ok(DummySnafu.fail()?)
        }

        #[test]
        fn it_works() {
            implements_error::<Opaque>();
            let _ = create();
        }
    }

    mod report {
        use snafu::prelude::*;

        #[derive(Debug, Snafu)]
        struct Error;

        #[test]
        #[snafu::report]
        fn it_works() -> Result<(), Error> {
            Ok(())
        }
    }
}

mod conditional_capabilities {
    use snafu::{AsErrorSource, ErrorCompat, IntoError, Snafu};
    use std::error::Error;

    #[derive(Debug, Snafu)]
    #[snafu(display("operation failed"), error_bounds(E: AsErrorSource))]
    struct Failure<E> {
        source: E,
    }

    #[derive(Debug, Snafu)]
    #[snafu(error_compat_bounds(), display_bounds(T: std::fmt::Display))]
    enum Recursive<T> {
        #[snafu(display("leaf: {value}"))]
        Leaf { value: T },
        #[snafu(display("nested: {source}"))]
        Nested {
            #[snafu(backtrace)]
            source: Box<Recursive<T>>,
        },
    }

    #[test]
    fn constructs_borrowed_payloads_without_diagnostic_traits() {
        struct Payload<'a>(&'a str);
        let text = String::from("payload");
        let error: Failure<_> = FailureSnafu.into_error(Payload(&text));
        assert_eq!(error.source.0, "payload");
        assert_eq!(error.to_string(), "operation failed");
    }

    #[test]
    fn recursive_capabilities_and_explicit_bounds_work() {
        let leaf: Recursive<i32> = LeafSnafu { value: 42 }.build();
        let error: Recursive<_> = NestedSnafu.into_error(Box::new(leaf));
        assert_eq!(error.to_string(), "nested: leaf: 42");
        assert!(error.source().unwrap().is::<Box<Recursive<i32>>>());
        assert!(error.backtrace().is_none());
    }
}
