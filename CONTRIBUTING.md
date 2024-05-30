# Contributing to Rust OCI Wasm

Thanks for your interest in contributing to Rust OCI Wasm! Our process for contribution is fairly
straightforward as this is a small project. Bug fixes and new features must be submitted using a
pull request. For new features, it is also recommended to first open an issue to discuss the feature
before submitting a pull request.

## Working with the code

The only required tooling outside of a standard development tool chain is 

- A valid [Rust installation](https://www.rust-lang.org/tools/install)
- Docker installed (for running integration tests)

As this is a wrapper around another crate, most of our tests are done via integration tests, which
can be found in the `tests/` directory. Please note that these tests are run as part of the CI 
pipeline, so they must pass before a pull request can be merged.

## Releasing the project

Releases are done by project maintainers as deemed necessary. Releases are entirely automated and
can be triggered by tagging the desired commit with a tag of the form `vX.Y.Z`. This will
automatically trigger the release process, which pushes the crate to crates.io.
