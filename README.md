# algorist

[![crates.io](https://img.shields.io/crates/d/cargo-algorist.svg)](https://crates.io/crates/cargo-algorist)
[![docs.rs](https://docs.rs/cargo-algorist/badge.svg)](https://docs.rs/cargo-algorist)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![dependencies](https://deps.rs/repo/github/farazdagi/cargo-algorist/status.svg)](https://deps.rs/repo/github/farazdagi/cargo-algorist)

Cargo sub-command to manage the [algorist](https://crates.io/crates/algorist) crate.

This crate is a CLI tool for managing programming contest projects AND a collection of useful
algorithms and data structures to use in those projects. It is aimed as a one-stop solution for
competitive programming in Rust.

## Installation

The crate provides cargo sub-command `algorist`, which can be installed using:

``` bash
cargo install cargo-algorist
```

Once installed, you can use it as `cargo algorist`.

## Usage

The `algorist` CLI tool provides a way to quickly create a new contest project, which is just a
normal Rust project, and allows to use additional modules with common algorithms and data
structures, work on problems, and when tests do pass, bundle each problem into a single output file
that can be submitted to the contest system.

### Create a new contest project

To create a new contest project:

``` bash
cargo algorist create <contest_id>

# examples:
cargo algorist create 4545
cargo algorist create contests/4545 # sub-folders are supported
```

This will create a Rust project with all the necessary problem files and algorithm modules copied
into it.

Problem files will be created in `src/bin` directory, and the library with algorithms and data
structures will be created in `crates/algorist` directory.

To see that everything works, you can run the problem file `src/bin/a.rs`:

``` bash
# run problem A (`src/bin/a.rs`)
# it expects input from stdin (type 1 2 3 and press Enter)
cargo run --bin a

# it is a normal Rust project, you can use all the usual commands
cargo build
cargo test --bin a
```

If you don't want to have initial problem files added to the contest project, you can create a new
contest project with `--empty` flag:

``` bash
cargo algorist create <contest_id> --empty
```

Later on, you can always add a problem file into `src/bin` directory, using:

``` bash
cargo algorist add <problem_id>

# examples:
cargo algorist add a        # `.rs` is not required
cargo algorist add a.rs     # same as above
```

### Work on a problem

All problems are located in `src/bin/<problem_id>.rs` files. The file will contain entry point
`main` function, which is expected to read input from standard input and write output to standard
output.

The starter code for the problem file will look something like this:

``` rust, no_run
use algorist::io::{test_cases, wln};

fn main() {
    test_cases(&mut |scan, w| {
        let (a, b) = scan.u2();
        wln!(w, "{}", a + b);
    });
}
```

See the [`documentation`](https://docs.rs/algorist/latest/algorist/algorist/) on `io` module (and
other provided algorithms and data structures) for more details on the default code provided in
problem files.

Normally, when working on solution, you copy the tests cases from the contest system into the
clipboard (or file), and then need to see the output of your program:

``` bash
# alias pbpaste=’xsel — clipboard — output’ on Linux
pbpaste | cargo run --bin <problem_id>   # gets input from clipboard
cargo run --bin <problem_id> < input.txt # gets input from file
```

Once you are happy with the output, you can submit the solution back to the contest system (by
bundling into a single file).

### Bundle the project

Contest systems expect a single output file, where all used modules are packed within the scope of
that file. At the very least `io` module is expected to be included in the output file.

``` bash
cargo algorist bundle <problem_id>

# examples:
cargo algorist bundle a # `.rs` is not required
cargo algorist bundle a.rs
```

This will create a single output file in `bundled/<problem_id>.rs` file, which can be submitted to
the contest system.

Only the modules actually used in the problem file will be included in the output file.

## Included algorithms and data structures

The Algorist is also a library of algorithms and data structures, which will be copied into your
contest project, and can be used in your problem files.

See [`algorist`](https://docs.rs/algorist/latest/algorist/algorist/) module documentation for a
complete list of available algorithms and data structures, as well as their usage examples.

## License

MIT
