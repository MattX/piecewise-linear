[![Build Status](https://travis-ci.org/MattX/piecewise-linear.svg?branch=master)](https://travis-ci.org/MattX/piecewise-linear)
[![Apache 2 licensed](https://img.shields.io/github/license/MattX/piecewise-linear)](LICENSE)
<!-- [![piecewise-linear on Crates.io](https://meritbadge.herokuapp.com/piecewise-linear)](https://crates.io/crates/piecewise-linear) -->

# piecewise-linear

[Documentation](https://mattx.github.io/piecewise-linear/doc/piecewise_linear/)

[View on GitHub](https://github.com/MattX/piecewise-linear)

## Piecewise linear function manipulation utilities

This crate provides utilities to manipulate continuous 
[piecewise linear functions](https://en.wikipedia.org/wiki/Piecewise_linear_function).
These are functions whose graph is made up of straight-line sections:

<img src="https://upload.wikimedia.org/wikipedia/commons/7/7c/Piecewise_linear_function.svg" alt="A piecewise linear function" width="25%">

It uses [geo](https://github.com/georust/geo) for geometric primitives and types.

### Usage example

```rust
let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
assert_eq!(f.y_at_x(1.25).unwrap(), 1.125);
```

### Features

- Efficient iterator over inflection points of _n_ functions
- Shrink and expand function domain 
- Sum
- Max
- Numerical integration
- Negation

Various convenience features are also implemented. See
[the documentation](https://mattx.github.io/piecewise-linear/doc/piecewise_linear/)
for more details. Pull requests for other features are very welcome!

### Other things to be done

- Improve CI with clippy and fmt
- Benchmarks
- Add links in rustdoc
- More tests

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
