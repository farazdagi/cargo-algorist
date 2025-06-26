## To Do

- [ ] `justfile` to minimize typing requirements (`just prob`, `just t prob`, `just a prob`,
  `just b prob`). With `just prob`, check if `inputs/a.txt` exists and if so interpret as
  `cargo run --bin a < inputs/a.txt`, otherwise run `cargo run --bin a`.

- [ ] Allow `pub use` re-exports (expose primes in math module and make sure that bundling works)
