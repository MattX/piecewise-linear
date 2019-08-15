// Copyright 2019 Matthieu Felix
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate geo;
extern crate num_traits;

#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::convert::{TryFrom, TryInto};

use geo::{Coordinate, CoordinateType, Line, LineString, Point};
use num_traits::Signed;

/// A continuous piecewise linear function.
///
/// The function is represented as a list of `(x, y)` pairs, each representing a point of
/// inflection (or equivalently a limit between two linear pieces). The represented function is
/// assumed to be linear between each of these points.
///
/// All methods defined on `PiecewiseLinearFunction` preserve the following invariants:
///
///   * There are at least two coordinates in the `coordinates` array
///   * The coordinates are in strictly increasing order of `x` value.
///
/// However, two consecutive segments do not necessarily have different slopes. These methods
/// will panic if invariants are broken by manually editing the `coordinates` vector.
///
/// This representation means that functions defined on an empty or singleton set, as well as
/// discontinuous functions, are not supported.
///
/// ```
/// use std::convert::TryFrom;
/// use piecewise_linear::PiecewiseLinearFunction;
/// let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
/// assert_eq!(f.y_at_x(1.25).unwrap(), 1.125);
/// ```
#[derive(PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PiecewiseLinearFunction<T: CoordinateType> {
    pub coordinates: Vec<Coordinate<T>>,
}

impl<T: CoordinateType> PiecewiseLinearFunction<T> {
    /// Creates a new `PiecewiseLinearFunction` from a vector of `Coordinates`.
    ///
    /// Returns a new PicewiseLinearFunction, or `None` if the invariants were not respected.
    pub fn new(coordinates: Vec<Coordinate<T>>) -> Option<Self> {
        if coordinates.len() >= 2 && coordinates.windows(2).all(|w| w[0].x < w[1].x) {
            Some(PiecewiseLinearFunction { coordinates })
        } else {
            None
        }
    }

    /// Returns a new constant `PiecewiseLinearFunction` with the specified domain and value, or
    /// `None` if the domain is not valid (i.e. `domain.1 <= domain.0`).
    pub fn constant(domain: (T, T), value: T) -> Option<Self> {
        if domain.0 < domain.1 {
            let coordinates = vec![(domain.0, value).into(), (domain.1, value).into()];
            Some(PiecewiseLinearFunction { coordinates })
        } else {
            None
        }
    }

    /// Returns a function's domain, represented as its min and max.
    pub fn domain(&self) -> (T, T) {
        (self.coordinates[0].x, self.coordinates.last().unwrap().x)
    }

    /// Checks whether this function has the same domain as another one.
    pub fn has_same_domain_as(&self, other: &PiecewiseLinearFunction<T>) -> bool {
        self.domain() == other.domain()
    }

    /// Returns an iterator over the segments of f. This iterator is guaranteed to have at least
    /// one element.
    pub fn segments_iter(&self) -> SegmentsIterator<T> {
        SegmentsIterator(self.coordinates.iter().peekable())
    }

    /// Returns an iterator over the joint points of inflection of `self` and `other`. See
    /// `points_of_inflection_iter` in this module for details.
    pub fn points_of_inflection_iter<'a>(
        &'a self,
        other: &'a PiecewiseLinearFunction<T>,
    ) -> Option<PointsOfInflectionIterator<T>> {
        if !self.has_same_domain_as(other) {
            None
        } else {
            Some(PointsOfInflectionIterator {
                segment_iterators: vec![
                    self.segments_iter().peekable(),
                    other.segments_iter().peekable(),
                ],
                heap: BinaryHeap::new(),
                initial: true,
            })
        }
    }

    /// Returns a segment `((x1, y1), (x2, y2))` of this function such that `x1 <= x <= x2`, or
    /// `None` if `x` is outside the domain of f.
    pub fn segment_at_x(&self, x: T) -> Option<Line<T>> {
        let idx = match self
            .coordinates
            .binary_search_by(|val| val.x.partial_cmp(&x).unwrap_or(Ordering::Equal))
        {
            Ok(idx) => idx,
            Err(idx) => {
                if idx == 0 || idx == self.coordinates.len() {
                    // Outside the function's domain
                    return None;
                } else {
                    idx
                }
            }
        };

        if idx == 0 {
            Some(Line::new(self.coordinates[idx], self.coordinates[idx + 1]))
        } else {
            Some(Line::new(self.coordinates[idx - 1], self.coordinates[idx]))
        }
    }

    /// Computes the value f(x) for this piecewise linear function. Returns `None` if `x` is
    /// outside the domain of f.
    pub fn y_at_x(&self, x: T) -> Option<T> {
        self.segment_at_x(x).map(|line| y_at_x(&line, x))
    }

    /// Returns a new piecewise linear function that is the restriction of this function to the
    /// specified domain.
    ///
    /// Returns `None` if `to_domain` is not a subset of the domain of `self`.
    pub fn shrink_domain(&self, to_domain: (T, T)) -> Option<PiecewiseLinearFunction<T>> {
        let order = order_domains(self.domain(), to_domain);
        match order {
            Some(Ordering::Equal) => Some(self.clone()),
            Some(Ordering::Greater) => {
                let mut new_points = Vec::new();
                if self.coordinates[0].x >= to_domain.0 {
                    new_points.push(self.coordinates[0]);
                }
                for segment in self.segments_iter() {
                    if let Some(restricted) = line_in_domain(&segment, to_domain) {
                        if segment.start.x < to_domain.0 {
                            new_points.push(restricted.start);
                        }
                        new_points.push(restricted.end);
                    }
                }
                Some(new_points.try_into().unwrap())
            }
            _ => None,
        }
    }

    /// Returns a new piecewise linear function that is the expansion of this function to the
    /// specified domain. At most one value is added on either side. See `ExpandDomainStrategy`
    /// for options determining how these added values are picked.
    pub fn expand_domain(
        &self,
        to_domain: (T, T),
        strategy: ExpandDomainStrategy,
    ) -> PiecewiseLinearFunction<T> {
        if order_domains(self.domain(), to_domain) == Some(Ordering::Equal) {
            return self.clone();
        }
        let mut new_points = Vec::new();
        if self.coordinates[0].x > to_domain.0 {
            match &strategy {
                ExpandDomainStrategy::ExtendSegment => new_points.push(Coordinate {
                    x: to_domain.0,
                    y: y_at_x(
                        &Line::new(self.coordinates[0], self.coordinates[1]),
                        to_domain.0,
                    ),
                }),
                ExpandDomainStrategy::ExtendValue => {
                    new_points.push((to_domain.0, self.coordinates[0].y).into());
                    new_points.push(self.coordinates[0]);
                }
            }
        } else {
            new_points.push(self.coordinates[0]);
        }

        let last_index = self.coordinates.len() - 1;
        new_points.extend_from_slice(&self.coordinates[1..last_index]);

        if self.coordinates[last_index].x < to_domain.1 {
            match &strategy {
                ExpandDomainStrategy::ExtendSegment => new_points.push(Coordinate {
                    x: to_domain.1,
                    y: y_at_x(
                        &Line::new(
                            self.coordinates[last_index - 1],
                            self.coordinates[last_index],
                        ),
                        to_domain.1,
                    ),
                }),
                ExpandDomainStrategy::ExtendValue => {
                    new_points.push(self.coordinates[last_index]);
                    new_points.push((to_domain.1, self.coordinates[last_index].y).into());
                }
            }
        } else {
            new_points.push(self.coordinates[last_index])
        }

        new_points.try_into().unwrap()
    }

    /// Sums this method with another piecewise linear function. Both functions must have the same
    /// domain.
    ///
    /// Returns None if the functions do not have the same domain.
    pub fn add(&self, other: &PiecewiseLinearFunction<T>) -> Option<PiecewiseLinearFunction<T>> {
        self.points_of_inflection_iter(other).map(|poi| {
            PiecewiseLinearFunction::new(
                poi.map(|(x, coords)| Coordinate {
                    x,
                    y: coords[0] + coords[1],
                })
                .collect(),
            )
            // This unwrap is guaranteed to succeed as the starting POI has generates ordered x,
            // which do not get modified.
            .unwrap()
        })
    }
}

/// Controls how the domain of a function is expanded using `expand_domain()` on
/// `PiecewiseLinearFunction`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExpandDomainStrategy {
    /// Extend the segment at the edge of the function.
    ExtendSegment,
    /// Add a constant segment with the value of the edge point of the function.
    ExtendValue,
}

impl<T: CoordinateType + Signed> PiecewiseLinearFunction<T> {
    /// Returns -f.
    pub fn negate(&self) -> PiecewiseLinearFunction<T> {
        PiecewiseLinearFunction::new(
            self.coordinates
                .iter()
                .map(|Coordinate { x, y }| Coordinate { x: *x, y: -(*y) })
                .collect(),
        )
        // This unwrap is guaranteed to succeed because the coordinate's x values haven't changed.
        .unwrap()
    }
}

impl<T: CoordinateType + Signed> ::std::ops::Neg for PiecewiseLinearFunction<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.negate()
    }
}

impl<T: CoordinateType + ::std::iter::Sum> PiecewiseLinearFunction<T> {
    /// Returns the integral of the considered function over its entire domain.
    pub fn integrate(&self) -> T {
        self.segments_iter()
            .map(|segment| {
                let (min_y, max_y) = if segment.start.y < segment.end.y {
                    (segment.start.y, segment.end.y)
                } else {
                    (segment.end.y, segment.start.y)
                };
                let x_span = segment.end.x - segment.start.x;
                x_span * (min_y + max_y / T::from(2).unwrap())
            })
            .sum()
    }
}

/**** Conversions ****/

impl<T: CoordinateType> TryFrom<LineString<T>> for PiecewiseLinearFunction<T> {
    type Error = ();

    fn try_from(value: LineString<T>) -> Result<Self, Self::Error> {
        PiecewiseLinearFunction::new(value.0).ok_or(())
    }
}

impl<T: CoordinateType> TryFrom<Vec<Coordinate<T>>> for PiecewiseLinearFunction<T> {
    type Error = ();

    fn try_from(value: Vec<Coordinate<T>>) -> Result<Self, Self::Error> {
        PiecewiseLinearFunction::new(value).ok_or(())
    }
}

impl<T: CoordinateType> TryFrom<Vec<Point<T>>> for PiecewiseLinearFunction<T> {
    type Error = ();

    fn try_from(value: Vec<Point<T>>) -> Result<Self, Self::Error> {
        PiecewiseLinearFunction::new(value.into_iter().map(|p| p.0).collect()).ok_or(())
    }
}

impl<T: CoordinateType> TryFrom<Vec<(T, T)>> for PiecewiseLinearFunction<T> {
    type Error = ();

    fn try_from(value: Vec<(T, T)>) -> Result<Self, Self::Error> {
        PiecewiseLinearFunction::new(value.into_iter().map(Coordinate::from).collect()).ok_or(())
    }
}

impl<T: CoordinateType> Into<Vec<(T, T)>> for PiecewiseLinearFunction<T> {
    fn into(self) -> Vec<(T, T)> {
        self.coordinates
            .into_iter()
            .map(|coord| coord.x_y())
            .collect()
    }
}

/**** Iterators ****/

#[derive(Debug, Clone, Copy, PartialEq)]
struct NextSegment<T: CoordinateType> {
    x: T,
    index: usize,
}

impl<T: CoordinateType> ::std::cmp::Eq for NextSegment<T> {}

impl<T: CoordinateType> ::std::cmp::PartialOrd for NextSegment<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.x.partial_cmp(&other.x).map(|r| r.reverse())
    }
}

impl<T: CoordinateType> ::std::cmp::Ord for NextSegment<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Structure returned by `points_of_inflection_iter()` on `PiecewiseLinearFunction`. See that
/// function's documentation for details.
pub struct PointsOfInflectionIterator<'a, T: CoordinateType + 'a> {
    segment_iterators: Vec<::std::iter::Peekable<SegmentsIterator<'a, T>>>,
    heap: BinaryHeap<NextSegment<T>>,
    initial: bool,
}

impl<'a, T: CoordinateType + 'a> PointsOfInflectionIterator<'a, T> {
    /// Helper method to avoid having rust complain about mutably accessing the segment iterators
    /// and heap at the same time.
    fn initialize(
        segment_iterators: &mut Vec<::std::iter::Peekable<SegmentsIterator<'a, T>>>,
        heap: &mut BinaryHeap<NextSegment<T>>,
    ) -> (T, Vec<T>) {
        let values = segment_iterators
            .iter_mut()
            .enumerate()
            .map(|(index, it)| {
                let seg = it.peek().unwrap();
                heap.push(NextSegment {
                    x: seg.end.x,
                    index,
                });
                seg.start.y
            })
            .collect();
        let x = segment_iterators[0].peek().unwrap().start.x;
        (x, values)
    }
}

impl<'a, T: CoordinateType + 'a> Iterator for PointsOfInflectionIterator<'a, T> {
    type Item = (T, Vec<T>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.initial {
            self.initial = false;
            Some(Self::initialize(
                &mut self.segment_iterators,
                &mut self.heap,
            ))
        } else {
            self.heap.peek().cloned().map(|next_segment| {
                let x = next_segment.x;
                let values = self
                    .segment_iterators
                    .iter_mut()
                    .map(|segment_iterator| y_at_x(segment_iterator.peek().unwrap(), x))
                    .collect();

                while let Some(segt) = self.heap.peek().cloned() {
                    if segt.x != x {
                        break;
                    }
                    self.heap.pop();
                    self.segment_iterators[segt.index].next();
                    if let Some(segment) = self.segment_iterators[segt.index].peek().cloned() {
                        self.heap.push(NextSegment {
                            x: segment.end.x,
                            index: segt.index,
                        })
                    }
                }

                (x, values)
            })
        }
    }
}

/// Structure returned by `segments_iter()` on a `PiecewiseLinearFunction`.
pub struct SegmentsIterator<'a, T: CoordinateType + 'a>(
    ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
);

impl<'a, T: CoordinateType + 'a> Iterator for SegmentsIterator<'a, T> {
    type Item = Line<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .and_then(|first| self.0.peek().map(|second| Line::new(*first, **second)))
    }
}

/**** General functions ****/

/// Returns an iterator over triples `(x, y1, y2)`, where `x` is the union of all points of
/// inflection of `self` and `other`, and `y1` and `y2` are the values of `self` and `other`,
/// respectively, at the corresponding `x`.
///
/// ```
/// use std::convert::TryFrom;
/// use piecewise_linear::{PiecewiseLinearFunction, points_of_inflection_iter};
/// let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
/// let g = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1.5, 3.), (2., 10.)]).unwrap();
/// assert_eq!(
///     points_of_inflection_iter(vec![f, g].as_slice()).unwrap().collect::<Vec<_>>(),
///     vec![(0., vec![0., 0.]), (1., vec![1., 2.]), (1.5, vec![1.25, 3.]), (2., vec![1.5, 10.])]
/// );
/// ```
///
/// The complexity of this method is _O(k log(k) n)_, where _k_ is the number of functions passed,
/// and _n_ is the number of points in each function.
pub fn points_of_inflection_iter<'a, T: CoordinateType + 'a>(
    funcs: &'a [PiecewiseLinearFunction<T>],
) -> Option<PointsOfInflectionIterator<'a, T>> {
    if !funcs.windows(2).all(|w| w[0].has_same_domain_as(&w[1])) {
        return None;
    }
    Some(PointsOfInflectionIterator {
        segment_iterators: funcs.iter().map(|f| f.segments_iter().peekable()).collect(),
        heap: BinaryHeap::new(),
        initial: true,
    })
}

/// Sums the functions together. Returns `None` in case of domain error.
///
/// This is faster than calling .add() repeatedly by a factor of _k / log(k)_.
pub fn sum<'a, T: CoordinateType + ::std::iter::Sum + 'a>(
    funcs: &[PiecewiseLinearFunction<T>],
) -> Option<PiecewiseLinearFunction<T>> {
    points_of_inflection_iter(funcs).map(|poi| {
        PiecewiseLinearFunction::new(
            poi.map(|(x, values)| Coordinate {
                x,
                y: values.iter().cloned().sum(),
            })
            .collect(),
        )
        // This unwrap is guaranteed to succeed because the coordinate's x values haven't changed.
        .unwrap()
    })
}

/**** Helpers ****/

/// Returns the restriction of segment `l` to the given domain, or `None` if the line's
/// intersection with the domain is either a singleton or empty.
fn line_in_domain<T: CoordinateType>(l: &Line<T>, domain: (T, T)) -> Option<Line<T>> {
    if l.end.x <= domain.0 || l.start.x >= domain.1 {
        None
    } else {
        let left_point = if l.start.x >= domain.0 {
            l.start
        } else {
            (domain.0, y_at_x(l, domain.0)).into()
        };
        let right_point = if l.end.x <= domain.1 {
            l.end
        } else {
            (domain.1, y_at_x(l, domain.1)).into()
        };
        Some(Line::new(left_point, right_point))
    }
}

fn y_at_x<T: CoordinateType>(line: &Line<T>, x: T) -> T {
    line.start.y + (x - line.start.x) * line.slope()
}

fn order_domains<T: CoordinateType>(d1: (T, T), d2: (T, T)) -> Option<Ordering> {
    if d1 == d2 {
        Some(Ordering::Equal)
    } else if d1.0 <= d2.0 && d1.1 >= d2.1 {
        Some(Ordering::Greater)
    } else if d2.0 <= d1.0 && d2.1 >= d1.1 {
        Some(Ordering::Less)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    fn get_test_function() -> PiecewiseLinearFunction<f64> {
        PiecewiseLinearFunction::try_from(vec![
            (-5.25, std::f64::MIN),
            (-std::f64::consts::FRAC_PI_2, 0.1),
            (-std::f64::consts::FRAC_PI_3, 0.1 + std::f64::EPSILON),
            (0.1, 1.),
            (1., 2.),
            (2., 3.),
            (3., 4.),
            (std::f64::INFINITY, std::f64::NEG_INFINITY),
        ])
        .unwrap()
    }

    #[test]
    fn test_y_at_x() {
        assert_eq!(y_at_x(&Line::new((0., 0.), (1., 1.)), 0.25), 0.25);
        assert_eq!(y_at_x(&Line::new((1., 0.), (2., 1.)), 1.25), 0.25);
    }

    #[test]
    fn test_constant() {
        assert_eq!(PiecewiseLinearFunction::constant((0.5, 0.5), 1.), None);
        assert_eq!(PiecewiseLinearFunction::constant((0.5, -0.5), 1.), None);
        assert_eq!(
            PiecewiseLinearFunction::constant((-25., -13.), 1.).unwrap(),
            vec![(-25., 1.), (-13., 1.)].try_into().unwrap()
        );
    }

    #[test]
    fn test_domain() {
        assert_eq!(
            PiecewiseLinearFunction::constant((-4., 5.25), 8.2)
                .unwrap()
                .domain(),
            (-4., 5.25)
        );
        assert_eq!(
            PiecewiseLinearFunction::try_from(vec![
                (std::f64::NEG_INFINITY, -1.),
                (0., 0.),
                (std::f64::INFINITY, 0.)
            ])
            .unwrap()
            .domain(),
            (std::f64::NEG_INFINITY, std::f64::INFINITY)
        );
    }

    #[test]
    fn test_segment_at_x() {
        assert_eq!(
            get_test_function().segment_at_x(1.5).unwrap(),
            Line::new((1., 2.), (2., 3.))
        );
        assert_eq!(
            get_test_function().segment_at_x(1.).unwrap(),
            Line::new((0.1, 1.), (1., 2.))
        );
    }

    #[test]
    fn test_segments_iter() {
        let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
        assert_eq!(
            f.segments_iter().collect::<Vec<_>>(),
            vec![
                Line::new((0., 0.), (1., 1.)),
                Line::new((1., 1.), (2., 1.5))
            ]
        );
    }

    #[test]
    fn test_points_of_inflection_iter() {
        let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
        let g = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1.5, 3.), (2., 10.)]).unwrap();
        assert_eq!(
            f.points_of_inflection_iter(&g).unwrap().collect::<Vec<_>>(),
            vec![
                (0., vec![0., 0.]),
                (1., vec![1., 2.]),
                (1.5, vec![1.25, 3.]),
                (2., vec![1.5, 10.])
            ]
        );
    }

    #[test]
    fn test_line_in_domain() {
        // Case 1 - fully outside
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (1., 2.)),
            None
        );
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (-3., -2.)),
            None
        );
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (0., 1.)),
            None
        );

        // Case 2 - fully inside
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (-2., 1.)),
            Some(Line::new((-1., 1.), (0., 2.)))
        );

        // Case 3 - overlap to the right
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (-0.5, 0.5)),
            Some(Line::new((-0.5, 1.5), (0., 2.)))
        );

        // Case 4 - overlap to the left
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (-1., -0.25)),
            Some(Line::new((-1., 1.), (-0.25, 1.75)))
        );

        // Case 5 - overlap on both sides
        assert_eq!(
            line_in_domain(&Line::new((-1., 1.), (0., 2.)), (-0.75, -0.25)),
            Some(Line::new((-0.75, 1.25), (-0.25, 1.75)))
        );
    }

    #[test]
    fn test_shrink_domain() {
        let first_val = y_at_x(
            &Line::new(
                (-std::f64::consts::FRAC_PI_3, 0.1 + std::f64::EPSILON),
                (0.1, 1.),
            ),
            0.,
        );
        assert_eq!(
            get_test_function()
                .shrink_domain((0.0, std::f64::INFINITY))
                .unwrap(),
            PiecewiseLinearFunction::try_from(vec![
                (0., first_val),
                (0.1, 1.),
                (1., 2.),
                (2., 3.),
                (3., 4.),
                (std::f64::INFINITY, std::f64::NEG_INFINITY),
            ])
            .unwrap()
        );
    }

    #[test]
    fn test_expand_domain() {
        let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();

        // Case 1: no expansion
        assert_eq!(
            f.expand_domain((0., 2.), ExpandDomainStrategy::ExtendSegment),
            f
        );

        // Case 2: left expansion
        assert_eq!(
            f.expand_domain((-1., 2.), ExpandDomainStrategy::ExtendSegment),
            vec![(-1., -1.), (1., 1.), (2., 1.5)].try_into().unwrap()
        );
        assert_eq!(
            f.expand_domain((-1., 2.), ExpandDomainStrategy::ExtendValue),
            vec![(-1., 0.), (0., 0.), (1., 1.), (2., 1.5)]
                .try_into()
                .unwrap()
        );

        // Case 3: right expansion
        assert_eq!(
            f.expand_domain((0., 4.), ExpandDomainStrategy::ExtendSegment),
            vec![(0., 0.), (1., 1.), (4., 2.5)].try_into().unwrap()
        );
        assert_eq!(
            f.expand_domain((0., 4.), ExpandDomainStrategy::ExtendValue),
            vec![(0., 0.), (1., 1.), (2., 1.5), (4., 1.5)]
                .try_into()
                .unwrap()
        );
    }

    #[test]
    fn test_negative() {
        let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
        assert_eq!(
            f.negate(),
            vec![(0., -0.), (1., -1.), (2., -1.5)].try_into().unwrap()
        )
    }
}
