use snafu::Snafu;
#[derive(Debug)]
struct Payload;
#[derive(Debug, Snafu)]
#[snafu(display("{value}"))]
struct Wrapped<T> { value: T }
fn main() {
    let error = Wrapped { value: Payload };
    println!("{}", error);
}
