use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(error_bounds(), error_bounds())]
struct DuplicateError;

#[derive(Debug, Snafu)]
#[snafu(error_compat_bounds(), error_compat_bounds())]
enum DuplicateCompat { Leaf }

fn main() {}
