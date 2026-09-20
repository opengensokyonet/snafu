use snafu::Snafu;
use std::fmt;

#[derive(Debug)]
struct Borrowed<'a>(&'a str);
impl fmt::Display for Borrowed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.0) }
}
impl std::error::Error for Borrowed<'_> {}

#[derive(Debug, Snafu)]
#[snafu(display("wrapped"))]
struct Wrapped<E> { source: E }

fn requires_error(_: impl std::error::Error) {}
fn main() {
    let text = String::from("borrowed");
    requires_error(Wrapped { source: Borrowed(&text) });
}
