use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(display("wrapped"), error_bounds())]
struct MissingSourceBound<T> { source: T }

#[derive(Debug, Snafu)]
#[snafu(display("delegated"), error_compat_bounds())]
struct MissingCompatBound<T> {
    #[snafu(backtrace)]
    source: T,
}

fn main() {}
