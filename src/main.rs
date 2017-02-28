#[macro_use]
extern crate error_chain;

mod interpreter;
mod errors;

fn main() {
    interpreter::run(&std::env::args().nth(1).expect("must supply script filename!")).unwrap();
}
