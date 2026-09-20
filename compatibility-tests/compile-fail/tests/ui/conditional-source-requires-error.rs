use snafu::Snafu;
use std::fmt;

#[derive(Debug)]
struct Payload;
impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("payload") }
}

#[derive(Debug, Snafu)]
#[snafu(display("wrapped"))]
struct Wrapped<E> { source: E }

fn requires_error(_: impl std::error::Error) {}
fn main() {
    requires_error(Wrapped { source: Payload });
}
