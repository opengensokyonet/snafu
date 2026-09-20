#![cfg_attr(
    feature = "unstable-provider-api",
    feature(error_generic_member_access)
)]

use snafu::{AsErrorSource, ErrorCompat, IntoError, Snafu};
use std::{error::Error, fmt, ops::Deref};

#[derive(Debug, Snafu)]
enum Recursive<T> {
    #[snafu(display("leaf: {value}"))]
    Leaf { value: T },
    #[snafu(display("nested: {source}"))]
    Nested {
        #[snafu(backtrace)]
        source: Box<Recursive<T>>,
    },
    #[snafu(display("qualified: {source}"))]
    QualifiedNested {
        #[snafu(backtrace)]
        source: Box<self::Recursive<T>>,
    },
}

#[test]
fn recursive_bounds_do_not_require_the_impl_being_generated() {
    let leaf: Recursive<i32> = LeafSnafu { value: 42 }.build();
    let nested: Recursive<_> = NestedSnafu.into_error(Box::new(leaf));
    assert_eq!(nested.to_string(), "nested: leaf: 42");
    let source = nested.source().unwrap();
    assert!(source.is::<Box<Recursive<i32>>>());
    assert!(source.source().is_none());
    assert!(nested.backtrace().is_none());
    let qualified: Recursive<_> = QualifiedNestedSnafu.into_error(Box::new(nested));
    assert_eq!(qualified.to_string(), "qualified: nested: leaf: 42");
    assert!(qualified.source().unwrap().is::<Box<Recursive<i32>>>());
    assert!(qualified.backtrace().is_none());

    let text = String::from("borrowed");
    let leaf: Recursive<&str> = LeafSnafu {
        value: text.as_str(),
    }
    .build();
    let nested: Recursive<_> = NestedSnafu.into_error(Box::new(leaf));
    assert_eq!(nested.to_string(), "nested: leaf: borrowed");

    struct Payload;
    let leaf: Recursive<Payload> = LeafSnafu { value: Payload }.build();
    let _: Recursive<_> = NestedSnafu.into_error(Box::new(leaf));
}

mod mutual {
    use super::*;

    #[derive(Debug, Snafu)]
    #[snafu(error_bounds(), error_compat_bounds(), display_bounds())]
    pub enum Left<T: fmt::Debug + fmt::Display + 'static> {
        #[snafu(display("leaf: {value}"))]
        Leaf { value: T },
        #[snafu(display("left: {source}"))]
        ToRight {
            #[snafu(backtrace)]
            source: Box<Right<T>>,
        },
    }

    #[derive(Debug, Snafu)]
    #[snafu(error_bounds(), error_compat_bounds(), display_bounds())]
    #[snafu(display("right: {source}"))]
    pub struct Right<T: fmt::Debug + fmt::Display + 'static> {
        #[snafu(backtrace)]
        source: Box<Left<T>>,
    }

    #[derive(Debug, Snafu)]
    #[snafu(error_bounds(), error_compat_bounds(), display_bounds())]
    #[snafu(source(from(exact)))]
    pub struct Public<T: fmt::Debug + fmt::Display + 'static>(Box<Left<T>>);

    #[test]
    fn explicit_bounds_break_mutual_cycles_and_preserve_delegation() {
        let leaf = Left::Leaf { value: 42 };
        let right = Right {
            source: Box::new(leaf),
        };
        let left = Left::ToRight {
            source: Box::new(right),
        };
        assert_eq!(left.to_string(), "left: right: leaf: 42");
        assert!(left.source().unwrap().is::<Box<Right<i32>>>());
        assert!(left
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<Box<Left<i32>>>());
        assert!(left.backtrace().is_none());
        let public: Public<_> = Box::new(left).into();
        assert_eq!(public.to_string(), "left: right: leaf: 42");
        assert!(public.source().unwrap().is::<Box<Right<i32>>>());
        assert!(public.backtrace().is_none());
    }
}

type Alias<T> = Box<T>;

#[derive(Debug, Snafu)]
#[snafu(display("aliased source"), error_bounds(T: AsErrorSource))]
struct Aliased<T: ?Sized> {
    source: Alias<T>,
}

#[derive(Debug)]
struct Pointer<'ctx, T: ?Sized> {
    value: Box<T>,
    label: &'ctx str,
}

impl<T: ?Sized> Deref for Pointer<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("custom pointer"), error_bounds(T: AsErrorSource))]
struct Indirect<'ctx, T: ?Sized> {
    source: Pointer<'ctx, T>,
}

#[test]
fn explicit_bounds_preserve_aliases_and_method_call_autoderef() {
    let source: Box<dyn Error> = Box::new(std::io::Error::new(std::io::ErrorKind::Other, "io"));
    let aliased: Aliased<dyn Error> = AliasedSnafu.into_error(source);
    assert!(aliased.source().unwrap().is::<std::io::Error>());

    let label = String::from("borrowed context");
    let source: Box<dyn Error> = Box::new(std::io::Error::new(std::io::ErrorKind::Other, "io"));
    let indirect: Indirect<'_, dyn Error> = IndirectSnafu.into_error(Pointer {
        value: source,
        label: &label,
    });
    assert_eq!(indirect.source.label, "borrowed context");
    assert!(indirect.source().unwrap().is::<std::io::Error>());

    struct Payload;
    let _: Aliased<Payload> = AliasedSnafu.into_error(Box::new(Payload));
}

mod shadowed {
    use super::*;

    // A user-defined Box need not delegate Error to its type parameter.
    #[derive(Debug)]
    struct Box<T>(T);
    impl<T> fmt::Display for Box<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("wrapper")
        }
    }
    impl<T: fmt::Debug> Error for Box<T> {}

    #[derive(Debug, Snafu)]
    #[snafu(display("shadowed pointer"), error_bounds(T: fmt::Debug + 'static))]
    struct Outer<T> {
        source: Box<T>,
    }

    #[test]
    fn override_replaces_standard_wrapper_spelling_inference() {
        let error: Outer<i32> = OuterSnafu.into_error(Box(42));
        assert!(error.source().unwrap().is::<Box<i32>>());
    }
}

mod sourced {
    use super::*;

    #[derive(Debug, Snafu)]
    enum Recursive<E> {
        #[snafu(display("base failure"))]
        Base { source: E },
        #[snafu(display("nested failure"))]
        Nested {
            #[snafu(source(from(Recursive<E>, Box::new)))]
            source: Box<Recursive<E>>,
            backtrace: snafu::Backtrace,
        },
    }

    #[test]
    fn recursion_retains_leaf_and_source_aware_construction_bounds() {
        let source = std::io::Error::new(std::io::ErrorKind::Other, "io");
        let base: Recursive<_> = BaseSnafu.into_error(source);
        let nested: Recursive<_> = NestedSnafu.into_error(base);
        let first = nested.source().unwrap();
        assert!(first.is::<Box<Recursive<std::io::Error>>>());
        assert!(first.source().unwrap().is::<std::io::Error>());
        assert!(nested.backtrace().is_some());
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("shared source"))]
struct Shared<E: ?Sized> {
    source: std::sync::Arc<E>,
}

#[derive(Debug, Snafu)]
#[snafu(display("local source"))]
struct Local<E: ?Sized> {
    source: std::rc::Rc<E>,
}

#[derive(Debug, Snafu)]
#[snafu(display("borrowed source"))]
struct Borrowed<'ctx, E: ?Sized> {
    source: &'ctx E,
}

#[test]
fn standard_pointer_sources_support_unsized_error_objects() {
    let source: std::sync::Arc<dyn Error> =
        std::sync::Arc::new(std::io::Error::new(std::io::ErrorKind::Other, "io"));
    let error: Shared<dyn Error> = SharedSnafu.into_error(source);
    assert!(error.source().unwrap().is::<std::io::Error>());

    let source: std::rc::Rc<dyn Error> =
        std::rc::Rc::new(std::io::Error::new(std::io::ErrorKind::Other, "io"));
    let error: Local<dyn Error> = LocalSnafu.into_error(source);
    assert!(error.source().unwrap().is::<std::io::Error>());

    let source = std::io::Error::new(std::io::ErrorKind::Other, "io");
    let error: Borrowed<'_, dyn Error> = BorrowedSnafu.into_error(&source as &dyn Error);
    assert!(error.source().unwrap().is::<std::io::Error>());
}

mod qualified {
    use super::*;

    mod other {
        use super::*;
        #[derive(Debug, Snafu)]
        #[snafu(display("other failure"))]
        pub struct Failure<T> {
            pub value: T,
        }
    }

    #[derive(Debug, Snafu)]
    #[snafu(display("outer failure"))]
    struct Failure<T> {
        source: other::Failure<T>,
    }

    #[test]
    fn a_qualified_type_with_the_same_name_is_not_self_recursion() {
        let error: Failure<u8> = FailureSnafu.into_error(other::Failure { value: 42 });
        assert!(error.source().unwrap().is::<other::Failure<u8>>());
    }
}
