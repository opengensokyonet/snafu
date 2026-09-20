use snafu::Snafu;

#[derive(Debug, Snafu)]
enum Invalid {
    #[snafu(error_bounds())]
    Leaf,
    #[snafu(error_compat_bounds())]
    Other,
}

#[derive(Debug, Snafu)]
struct Fields {
    #[snafu(error_bounds())]
    value: u8,
    #[snafu(error_compat_bounds())]
    other: u8,
}

#[derive(Debug, Snafu)]
struct Tuple(#[snafu(error_bounds(), error_compat_bounds())] std::io::Error);

fn main() {}
