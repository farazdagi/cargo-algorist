# algorist

[![crates.io](https://img.shields.io/crates/d/cargo-algorist.svg)](https://crates.io/crates/cargo-algorist)
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

See the [`documentation`](https://docs.rs/algorist/latest/algorist/) on `io` module (and other
provided algorithms and data structures of [algorist](https://crates.io/crates/algorist) crate) for
more details on the default code provided in problem files.

Normally, when working on solution, you copy the tests cases from the contest system into the
clipboard (or file), and then need to see the output of your program. With the project created using
`algorist`, you can do this easily:

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

The Algorist library contains a lot of useful code that can be imported into your contest projects.

See [`algorist`](https://docs.rs/algorist/latest/algorist/) crate documentation for a complete list
of available algorithms and data structures, as well as their usage examples.

## Using your own algorithms and data structures

While the Algorist crate provides a lot of useful algorithms and data structures, the original plan
was to allow users to also rely on their own code, i.e. you are working on contest problems, and
when you find something that can be abstracted into a reusable module, you can do it and expand your
own library of algorithms.

By default, when creating projects with `cargo algorist create` the
[`algorist`](https://docs.rs/algorist/latest/algorist/) library will be added into `crates/algorist`
directory.

If you want to use your own library, specify path to it using `--manifest-path`:

``` bash
cargo algorist create <contest_id> --manifest-path /path/to/your/lib/Cargo.toml

# Path to project (directory with `Cargo.toml`) will also work
cargo algorist create <contest_id> --manifest-path /path/to/your/lib
```

Your project will be created in the same way, but instead of copying the
[`algorist`](https://docs.rs/algorist/latest/algorist/) library, it will use your own library.

It is recommended that you start by forking the [`algorist`](https://github.com/farazdagi/algorist)
repository, and then use it as a base for your own library (remove everything, but `io`, and have
fun).

Note that default problem files will assume that at least `io` module is available, so you will need
to provide it in your own library.

## License

MIT
