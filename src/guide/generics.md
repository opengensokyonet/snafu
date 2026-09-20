# Using generic types

Error types enhanced by SNAFU may contain generic type and lifetime parameters.

Construction and diagnostic capabilities have independent bounds.

## Types

```rust
# use snafu::prelude::*;
#
#[derive(Debug, Snafu)]
enum Error<T> {
    #[snafu(display("The value {value} was too large"))]
    TooLarge { value: T, limit: u32 },

    #[snafu(display("The value {value} was too small"))]
    TooSmall { value: T, limit: u32 },
}

fn validate_number(value: u8) -> Result<u8, Error<u8>> {
    ensure!(
        value <= 200,
        TooLargeSnafu {
            value,
            limit: 100u32,
        },
    );
    ensure!(
        value >= 100,
        TooSmallSnafu {
            value,
            limit: 200u32,
        },
    );
    Ok(value)
}

fn validate_string(value: &str) -> Result<&str, Error<String>> {
    ensure!(
        value.len() <= 20,
        TooLargeSnafu {
            value,
            limit: 10u32,
        },
    );
    ensure!(
        value.len() >= 10,
        TooSmallSnafu {
            value,
            limit: 20u32,
        },
    );
    Ok(value)
}
```

## Lifetimes

```rust
# use snafu::prelude::*;
#
#[derive(Debug, Snafu)]
enum Error<'a> {
    #[snafu(display("The username {value} contains the bad word {word}"))]
    BadWord { value: &'a str, word: &'static str },
}

fn validate_username<'a>(value: &'a str) -> Result<&'a str, Error<'a>> {
    ensure!(
        !value.contains("stinks"),
        BadWordSnafu {
            value,
            word: "stinks",
        },
    );
    ensure!(
        !value.contains("smells"),
        BadWordSnafu {
            value,
            word: "smells",
        },
    );
    Ok(value)
}
```

## Conditional diagnostic capabilities

A generic source does not need to implement `Error`, `Debug`, or
`Display` merely to be stored, matched, or wrapped with context:

```rust
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
#[snafu(display("read failed after {completed} bytes"))]
struct ReadFailure<E> {
    source: E,
    completed: usize,
}

fn attach<T, E>(result: Result<T, E>) -> Result<T, ReadFailure<E>> {
    result.context(ReadFailureSnafu { completed: 3usize })
}

struct Borrowed<'a>(&'a str); // No diagnostic traits.
let text = String::from("device failure");
let error = attach::<(), _>(Err(Borrowed(&text))).err().unwrap();
assert_eq!(error.to_string(), "read failed after 3 bytes");
assert_eq!(error.source.0, "device failure");

use std::error::Error;
let source = std::io::Error::new(std::io::ErrorKind::Other, "device failure");
let error = attach::<(), _>(Err(source)).err().unwrap();
assert!(error.source().unwrap().is::<std::io::Error>());
```

The generated `Error` implementation requires `Self: Debug + Display`
and the ability to expose each source through `AsErrorSource`. A generic
value source normally needs `Error + 'static`. Non-static sources can
still be carried as typed fields; this does not change the standard
`Error::source` signature. Borrowed context fields do not independently
need to be `'static` for the outer type to implement `Error`.

`Display` is independently conditional on its formatting requirements.
Use [`display_bounds`](crate::Snafu#controlling-display-bounds) to replace
inference for complex expressions. `Debug` remains controlled by its
own implementation: standard `derive(Debug)` can add conservative bounds,
for example on a host parameter when the field is an associated type.

`ErrorCompat` also has independent requirements. Retrieving a local
backtrace need not constrain the source, whereas delegating to a source
requires its `ErrorCompat` implementation. Iterating the standard error
chain additionally requires `AsErrorSource`.

### Source-aware implicit data

Constructors requesting a backtrace or another implicit field still use
`GenerateImplicitData::generate_with_source(&dyn Error)` when there is a
source. They retain the required source capability. This exception
preserves source-aware generation; the macro does not silently switch to
`generate()` when a source cannot be exposed as a standard error.

### Opaque wrappers

A generic [opaque wrapper](crate::guide::opaque) conditionally delegates
`Display`, `Error`, and `ErrorCompat` to the inner type. Construction need
not require any of these capabilities:

```rust
use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(source(from(exact)))]
struct ApiError<E>(E);

struct Payload;
let _: ApiError<Payload> = Payload.into();
```

The existing `source(from(exact))` option avoids a potentially overlapping
generic `From` implementation. Exposing a generic opaque type still
exposes its type parameter as part of the public API.

### Inference boundaries

Source-bound inference recognizes references and standard
`Box`/`Rc`/`Arc` spellings, plus the `Option` used by `whatever`. It does not
perform name resolution or arbitrary autoderef analysis. Custom smart
pointers, aliases, and recursive generic sources are not covered by
these inference guarantees.
