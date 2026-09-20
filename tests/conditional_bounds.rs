#![cfg_attr(
    feature = "unstable-provider-api",
    feature(error_generic_member_access)
)]

//! Constructing a typed failure is independent of reporting it.
use snafu::{ErrorCompat, IntoError, OptionExt, ResultExt, Snafu};
use std::{error::Error, fmt};

struct Raw<'a>(&'a str); // Intentionally no Debug, Display, or Error.

#[derive(Debug, Snafu)]
enum ReadFailure<E> {
    #[snafu(display("read failed after {completed} bytes"))]
    Host { source: E, completed: usize },
    #[snafu(display("unexpected EOF after {completed} bytes"))]
    Eof { completed: usize },
}

fn attach<T, E>(result: Result<T, E>) -> Result<T, ReadFailure<E>> {
    result.context(HostSnafu { completed: 3usize })
}

#[test]
fn arbitrary_borrowed_source_can_be_constructed_and_displayed() {
    let text = String::from("borrowed failure");
    let error = attach::<(), _>(Err(Raw(&text))).err().unwrap();
    assert_eq!(error.to_string(), "read failed after 3 bytes");
    assert!(ErrorCompat::backtrace(&error).is_none());
    match error {
        ReadFailure::Host {
            source: Raw(s),
            completed,
        } => {
            assert_eq!(s, text);
            assert_eq!(completed, 3);
        }
        _ => panic!("wrong variant"),
    }
    let error: ReadFailure<Raw<'_>> = EofSnafu { completed: 2usize }.build();
    assert_eq!(error.to_string(), "unexpected EOF after 2 bytes");
    let error: Result<(), ReadFailure<Raw<'_>>> = None.context(EofSnafu { completed: 0usize });
    assert!(matches!(error, Err(ReadFailure::Eof { completed: 0 })));
    let error = Err::<(), _>(Raw(&text)).with_context(|_| HostSnafu { completed: 4usize });
    assert!(matches!(error, Err(ReadFailure::Host { completed: 4, .. })));
    let error: Result<(), ReadFailure<Raw<'_>>> =
        None.with_context(|| EofSnafu { completed: 1usize });
    assert!(error.is_err());
}

#[test]
fn standard_sources_keep_the_source_chain() {
    let error = attach::<(), _>(Err(std::io::Error::other("device failed")))
        .err()
        .unwrap();
    assert!(error.source().unwrap().is::<std::io::Error>());
    let outer = attach::<(), _>(Err(error)).err().unwrap();
    assert!(outer
        .source()
        .unwrap()
        .source()
        .unwrap()
        .is::<std::io::Error>());
    assert_eq!(ErrorCompat::iter_chain(&outer).count(), 3);
}

#[derive(Debug, Snafu)]
#[snafu(display("{value} {value:#?} {hex:#x}"))]
struct Values<T, H> {
    value: T,
    hex: H,
}

#[test]
fn formatting_bounds_do_not_restrict_construction() {
    let _value: Values<Raw<'_>, Raw<'_>> = ValuesSnafu {
        value: Raw("a"),
        hex: Raw("b"),
    }
    .build();
    let value = ValuesSnafu {
        value: "label",
        hex: 15u8,
    }
    .build::<&str, u8>();
    assert_eq!(value.to_string(), "label \"label\" 0xf");
    fn assert_error<E: Error>(_: &E) {}
    assert_error(&value);
}

trait Describe {
    type Description;
    fn describe(&self) -> Self::Description;
}
struct Described;
impl Describe for Described {
    type Description = &'static str;
    fn describe(&self) -> &'static str {
        "description"
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("{}", value.describe()), display_bounds(T: Describe, T::Description: fmt::Display))]
struct Description<T> {
    value: T,
}

#[derive(Debug, Snafu)]
#[snafu(display(concat!("constant", " message")), display_bounds())]
struct ExplicitEmpty<T> {
    value: T,
}

#[test]
fn explicit_bounds_replace_inference_including_an_empty_list() {
    let error: Description<Described> = DescriptionSnafu { value: Described }.build();
    assert_eq!(error.to_string(), "description");
    let _: Description<Raw<'_>> = DescriptionSnafu { value: Raw("raw") }.build();
    let error: ExplicitEmpty<Raw<'_>> = ExplicitEmptySnafu { value: Raw("raw") }.build();
    assert_eq!(error.to_string(), "constant message");
}

#[derive(Debug, Snafu)]
#[snafu(display_bounds(T: fmt::Display))]
enum EnumDisplay<T> {
    #[snafu(display("{value}"))]
    Value { value: T },
    #[snafu(display("empty"))]
    Empty,
}

#[test]
fn enum_bounds_apply_to_the_whole_display_impl() {
    let e = ValueSnafu { value: "value" }.build::<&str>();
    assert_eq!(e.to_string(), "value");
    let _: EnumDisplay<Raw<'_>> = EmptySnafu.build();
}

trait Host {
    type Failure;
}
struct Device;
impl Host for Device {
    type Failure = std::io::Error;
}

#[derive(Snafu)]
#[snafu(display("host failed"))]
struct Associated<H: Host> {
    source: H::Failure,
}
// Standard derive(Debug) would add H: Debug, unrelated to the source's type.
impl<H: Host> fmt::Debug for Associated<H>
where
    H::Failure: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Associated").field(&self.source).finish()
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("boxed source"))]
struct Boxed<E: ?Sized> {
    source: Box<E>,
}

#[test]
fn associated_and_boxed_dyn_sources_remain_supported() {
    let e: Associated<Device> = AssociatedSnafu.into_error(std::io::Error::other("io"));
    assert!(e.source().unwrap().is::<std::io::Error>());
    let e: Boxed<dyn Error> =
        BoxedSnafu.into_error(Box::new(std::io::Error::other("io")) as Box<dyn Error>);
    assert!(e.source().unwrap().is::<std::io::Error>());
}

#[derive(Debug, Snafu)]
#[snafu(transparent)]
struct Transparent<E> {
    source: E,
}

#[derive(Debug, Snafu)]
#[snafu(source(from(exact)))]
struct Opaque<E>(E);

#[derive(Debug, Snafu)]
#[snafu(display("converted"))]
struct Converted<E> {
    #[snafu(source(from(E, Box::new)))]
    source: Box<E>,
}

#[test]
fn transparent_opaque_and_transformed_construction_is_unconstrained() {
    let _: Transparent<Raw<'_>> = Raw("a").into();
    let _: Opaque<Raw<'_>> = Raw("a").into();
    let e: Converted<Raw<'_>> = ConvertedSnafu.into_error(Raw("a"));
    assert_eq!(e.source.0, "a");
    let e: Converted<std::io::Error> = ConvertedSnafu.into_error(std::io::Error::other("io"));
    assert!(e.source().unwrap().is::<std::io::Error>());
    let inner = attach::<(), _>(Err(std::io::Error::other("io")))
        .err()
        .unwrap();
    let e: Transparent<_> = inner.into();
    assert!(e.source().unwrap().is::<std::io::Error>());
    let e: Opaque<_> = attach::<(), _>(Err(std::io::Error::other("io")))
        .err()
        .unwrap()
        .into();
    assert!(e.source().unwrap().is::<std::io::Error>());
}

struct OnlyCompat;
impl ErrorCompat for OnlyCompat {}
#[derive(Debug, Snafu)]
#[snafu(display("delegated"))]
struct Delegated<E> {
    #[snafu(backtrace)]
    source: E,
}
#[test]
fn backtrace_compatibility_is_independent_of_error() {
    let e: Delegated<OnlyCompat> = DelegatedSnafu.into_error(OnlyCompat);
    assert!(ErrorCompat::backtrace(&e).is_none());
    let _: Delegated<Raw<'_>> = DelegatedSnafu.into_error(Raw("raw"));
}

#[derive(Debug, Snafu)]
#[snafu(display("{{{1:.*}}} {value:x?} {value:#X?}", precision, value))]
struct FormatCursor<T> {
    value: T,
    precision: usize,
}
#[derive(Debug, Snafu)]
#[snafu(display("{value:width$.precision$}", width = width, precision = precision))]
struct DynamicWidth<T> {
    value: T,
    width: usize,
    precision: usize,
}
#[derive(Debug, Snafu)]
#[snafu(display("{value:0$}", width))]
struct WidthZero<T> {
    value: T,
    width: usize,
}
#[derive(Debug, Snafu)]
#[snafu(display("{value:p}"))]
struct Pointer<T> {
    value: T,
}

#[test]
fn format_parameter_mapping_and_borrowed_pointer_semantics() {
    let e: FormatCursor<f64> = FormatCursorSnafu {
        value: 1.2345,
        precision: 2usize,
    }
    .build();
    assert_eq!(e.to_string(), "{1.23} 1.2345 1.2345");
    let e: DynamicWidth<f64> = DynamicWidthSnafu {
        value: 1.2345,
        width: 6usize,
        precision: 2usize,
    }
    .build();
    assert_eq!(e.to_string(), "  1.23");
    let e: WidthZero<&str> = WidthZeroSnafu {
        value: "a",
        width: 3usize,
    }
    .build();
    assert_eq!(e.to_string(), "a  ");
    let e: Pointer<Raw<'_>> = PointerSnafu { value: Raw("raw") }.build();
    assert!(e.to_string().starts_with("0x"));
}

#[test]
fn construction_protocol_does_not_require_error_compat() {
    struct Unreported<E>(E);
    struct Context;
    impl<E> IntoError<Unreported<E>> for Context {
        type Source = E;
        fn into_error(self, source: E) -> Unreported<E> {
            Unreported(source)
        }
    }
    let error = Err::<(), _>(Raw("raw")).context(Context).err().unwrap();
    assert_eq!(error.0 .0, "raw");
}

#[derive(Debug)]
struct BorrowedError<'a>(&'a str);
impl fmt::Display for BorrowedError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl Error for BorrowedError<'_> {}

#[test]
fn non_static_error_is_a_valid_payload() {
    let text = String::from("borrowed error");
    let error = attach::<(), _>(Err(BorrowedError(&text))).err().unwrap();
    assert_eq!(error.to_string(), "read failed after 3 bytes");
    assert!(format!("{error:?}").contains("borrowed error"));
}

#[derive(Debug)]
struct SourceMarker;
impl snafu::GenerateImplicitData for SourceMarker {
    fn generate() -> Self {
        panic!("must preserve generate_with_source")
    }
    fn generate_with_source(source: &dyn Error) -> Self {
        assert_eq!(source.to_string(), "original");
        Self
    }
}
#[derive(Debug, Snafu)]
#[snafu(display("implicit context"))]
struct Implicit<E> {
    source: E,
    #[snafu(implicit)]
    marker: SourceMarker,
}
#[test]
fn implicit_data_keeps_source_aware_generation() {
    let error: Implicit<_> = ImplicitSnafu.into_error(std::io::Error::other("original"));
    assert_eq!(error.source().unwrap().to_string(), "original");
}

#[cfg(feature = "futures")]
mod asynchronous {
    use super::*;
    use futures_core::Stream;
    use snafu::futures::{TryFutureExt, TryStreamExt};
    use std::{
        future::{ready, Future},
        pin::Pin,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };

    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    struct Once<'a>(Option<Raw<'a>>);
    impl<'a> Stream for Once<'a> {
        type Item = Result<(), Raw<'a>>;
        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Ready(self.0.take().map(Err))
        }
    }
    #[test]
    fn future_and_stream_context_accept_non_debug_borrowed_sources() {
        let text = String::from("async borrowed");
        let waker = Waker::from(Arc::new(Noop));
        let mut cx = Context::from_waker(&waker);
        let mut future =
            Box::pin(ready(Err::<(), _>(Raw(&text))).context(HostSnafu { completed: 5usize }));
        assert!(matches!(
            future.as_mut().poll(&mut cx),
            Poll::Ready(Err(ReadFailure::Host { completed: 5, .. }))
        ));
        let mut future = Box::pin(
            ready(Err::<(), _>(Raw(&text))).with_context(|_| HostSnafu { completed: 6usize }),
        );
        assert!(matches!(
            future.as_mut().poll(&mut cx),
            Poll::Ready(Err(ReadFailure::Host { completed: 6, .. }))
        ));
        let mut stream = Box::pin(Once(Some(Raw(&text))).context(HostSnafu { completed: 7usize }));
        assert!(matches!(
            stream.as_mut().poll_next(&mut cx),
            Poll::Ready(Some(Err(ReadFailure::Host { completed: 7, .. })))
        ));
        assert!(matches!(
            stream.as_mut().poll_next(&mut cx),
            Poll::Ready(None)
        ));
        let mut stream =
            Box::pin(Once(Some(Raw(&text))).with_context(|_| HostSnafu { completed: 8usize }));
        assert!(matches!(
            stream.as_mut().poll_next(&mut cx),
            Poll::Ready(Some(Err(ReadFailure::Host { completed: 8, .. })))
        ));
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("source: {source:?}; operation: {operation}"))]
struct SourceDisplay<E, O> {
    source: E,
    operation: O,
}

#[derive(Debug)]
struct Constant<T>(T);
impl<T> fmt::Display for Constant<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("constant")
    }
}
#[derive(Debug, Snafu)]
#[snafu(display("{value}"))]
struct WholeField<T> {
    value: Constant<T>,
}

#[test]
fn display_constraints_follow_used_field_types() {
    let e: SourceDisplay<u8, &str> = SourceDisplaySnafu { operation: "read" }.into_error(7);
    assert_eq!(e.to_string(), "source: 7; operation: read");
    let e: WholeField<Raw<'_>> = WholeFieldSnafu {
        value: Constant(Raw("raw")),
    }
    .build();
    assert_eq!(e.to_string(), "constant");
    let label = String::from("borrowed context");
    let e: SourceDisplay<std::io::Error, &str> = SourceDisplaySnafu {
        operation: label.as_str(),
    }
    .into_error(std::io::Error::other("io"));
    // Only the source needs to support 'static error erasure, not the context.
    assert!(e.source().unwrap().is::<std::io::Error>());
}
