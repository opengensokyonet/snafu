use snafu::Snafu;
#[derive(Debug, Snafu)]
enum Invalid {
    #[snafu(display_bounds())]
    Variant,
}
fn main() {}
