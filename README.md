[![Build Status](https://travis-ci.org/MattX/piecewise-linear.svg?branch=master)](https://travis-ci.org/MattX/piecewise-linear)
<!-- [![piecewise-linear on Crates.io](https://meritbadge.herokuapp.com/piecewise-linear)](https://crates.io/crates/piecewise-linear) -->

# piecewise-linear

## Piecewise linear function manipulation utilities

This crate provides utilities to manipulate continuous functions that
can be represented as a collection of linear functions, each operating
on a disjoint domain.

It uses [geo](https://github.com/georust/geo) for geometric primitives
and types.

### Features

- [x] Efficient iterator over inflection points of two functions
- [x] Shrink and expand function domain 
- [x] Negation
- [x] Numerical integration
- [ ] Sum
- [ ] Min / max
- [ ] Product
- [ ] Abs value

Most numeric types should be supported, but only `f64` has been
seriously tested.

See [the documentation](https://mattx.github.io/piecewise-linear/piecewise_linear/)
for more details.

### Other todo

- Improve CI with clippy and fmt
- Submit crate
- Benchmarks
- Home page in GH pages deployment
- Add links in rustdoc? Unclear how that works
- Split lib.rs

## Contributing

Feel free to open issues and pull requests! Documentation improvements
are appreciated. Please fully document new public features and provide
a unit testing suite for any new code.

## License

Licensed under the Apache-2.0 license. See the LICENSE file for details.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be licensed as above, without
any additional terms or conditions.
