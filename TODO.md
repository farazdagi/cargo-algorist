## To Do

- [ ] `justfile` to minimize typing requirements (`just prob`, `just t prob`, `just a prob`,
  `just b prob`). With `just prob`, check if `inputs/a.txt` exists and if so interpret as
  `cargo run --bin a < inputs/a.txt`, otherwise run `cargo run --bin a`.

- [ ] Consider adding `inputs/<problem_id>.txt` files to the project. Both on non-empty project
  creation and problem add command.

- [ ] Provide `examples`, with a CodeForces contest as an example.

- [ ] Allow `pub use` re-exports (expose primes in math module and make sure that bundling works)

- [x] Allow to use external `algos` lib -- implement `--manifest-path path/to/Cargo.toml` flag

- [x] How to contribute (and develop your own version of library).

