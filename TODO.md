## To Do

- [ ] `justfile` to minimize typing requirements (`just prob`, `just t prob`, `just a prob`,
  `just b prob`). With `just prob`, check if `inputs/a.txt` exists and if so interpret as
  `cargo run --bin a < inputs/a.txt`, otherwise run `cargo run --bin a`.

- [ ] Allow `pub use` re-exports (expose primes in math module and make sure that bundling works)

- [ ] Add `.algorist` config file functionality. At the very least, users will be able to specify
  the path to their custom library, and will not have to specify it using the `manifest-path` flag.
