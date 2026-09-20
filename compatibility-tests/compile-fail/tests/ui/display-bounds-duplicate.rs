use snafu::Snafu;
#[derive(Debug, Snafu)]
#[snafu(display_bounds(), display_bounds())]
struct Invalid;
fn main() {}
