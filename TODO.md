## To Do

- [ ] `justfile` to minimize typing requirements (`just prob`, `just t prob`, `just a prob`,
  `just b prob`). With `just prob`, check if `inputs/a.txt` exists and if so interpret as
  `cargo run --bin a < inputs/a.txt`, otherwise run `cargo run --bin a`.

- [ ] Consider adding `inputs/<problem_id>.txt` files to the project. Both on non-empty project
  creation and problem add command.

- [ ] Document existing algorithms and data structures.
  
  - [x] `math`
    - [x] `primes`
    - [x] `gcd`
    - [x] `root`
    - [x] `modulo`
    - [x] `log`
  - [ ] `ext`
  - [x] `collections`
  - [ ] `misc`

- [ ] Provide `examples`, with a CodeForces contest as an example.

- [ ] Plan extension of the library with more algorithms and data structures.

- [ ] Allow `pub use` re-exports (expose primes in math module and make sure that bundling works)

- [ ] Allow to use external `algos` lib -- ensure that `include_dir` compatible template source is
  used.
  
  - [ ] How to contribute (and develop your own version of library).
