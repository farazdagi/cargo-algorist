## To Do

- [x] Allow `pub use` re-exports (expose primes in math module and make sure that bundling works)

- [ ] Add `.algorist` config file functionality. At the very least, users will be able to specify
  the path to their custom library, and will not have to specify it using the `manifest-path`
  flag.

- [ ] Make sure that modules in library car refer each other: during the first phase, collect
  imports from the binary file, then descent into each imported library module, and recursively
  collect there as well, in the end -- usage tree will hold all the necessary modules. Make sure
  that `pub use` declarations usage is also updated -- otherwise indirectly referred modules
  will not be included.

- [ ] Add flag to `algorist run bundled`
