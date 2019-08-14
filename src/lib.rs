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

#[macro_use]
extern crate geo;

use std::cmp::Ordering;
use std::convert::{TryFrom, TryInto};

use geo::{Coordinate, CoordinateType, Line, LineString, Point};

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
/// However, two consecutive segments do not necessarily have different slopes.
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
    /// Creates a new [PiecewiseLinearFunction] from a vector of [Coordinate]s.
    ///
    /// Returns a new PicewiseLinearFunction, or `None` if the invariants were not respected.
    pub fn new(coordinates: Vec<Coordinate<T>>) -> Option<Self> {
        if coordinates.len() >= 2 && coordinates.windows(2).all(|w| w[0].x < w[1].x) {
            Some(PiecewiseLinearFunction { coordinates })
        } else {
            None
        }
    }

    /// Returns a new constant [PiecewiseLinearFunction] with the specified domain and value, or
    /// `None` if the domain is not valid (i.e. `domain.1 <= domain.0`).
    pub fn constant(domain: (T, T), value: T) -> Option<Self> {
        if domain.0 < domain.1 {
            let coordinates = vec![(domain.0, value).into(), (domain.1, value).into()];
            Some(PiecewiseLinearFunction { coordinates })
        } else {
            None
        }
    }

    /// Returns the minimum and maximum of a function's domain.
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

    /// Returns an iterator over triples `(x, y1, y2)`, where `x` is the union of all points of
    /// inflection of `self` and `other`, and `y1` and `y2` are the values of `self` and `other`,
    /// respectively, at the corresponding `x`.
    ///
    /// ```
    /// use std::convert::TryFrom;
    /// use piecewise_linear::PiecewiseLinearFunction;
    /// let f = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1., 1.), (2., 1.5)]).unwrap();
    /// let g = PiecewiseLinearFunction::try_from(vec![(0., 0.), (1.5, 3.), (2., 10.)]).unwrap();
    /// assert_eq!(
    ///     f.points_of_inflection_iter(&g).unwrap().collect::<Vec<_>>(),
    ///     vec![(0., 0., 0.), (1., 1., 2.), (1.5, 1.25, 3.), (2., 1.5, 10.)]
    /// );
    /// ```
    pub fn points_of_inflection_iter<'a>(
        &'a self,
        other: &'a PiecewiseLinearFunction<T>,
    ) -> Option<PointsOfInflectionIterator<T>> {
        if !self.has_same_domain_as(other) {
            None
        } else {
            Some(PointsOfInflectionIterator {
                first: self.coordinates.iter().peekable(),
                second: other.coordinates.iter().peekable(),
                first_segment_iterator: self.segments_iter().peekable(),
                second_segment_iterator: other.segments_iter().peekable(),
                initial: true,
            })
        }
    }

    /// Returns a segment ((x1, y1), (x2, y2)) of f such that x1 <= x <= x2, or `None` if `x` is
    /// outside the domain of f.
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
}

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
        PiecewiseLinearFunction::new(
            value
                .into_iter()
                .map(|tuple| Coordinate::from(tuple))
                .collect(),
        )
        .ok_or(())
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

/// Structure returned by [PiecewiseLinearFunction::points_of_inflection_iter].
pub struct PointsOfInflectionIterator<'a, T: CoordinateType + 'a> {
    first: ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
    second: ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
    first_segment_iterator: ::std::iter::Peekable<SegmentsIterator<'a, T>>,
    second_segment_iterator: ::std::iter::Peekable<SegmentsIterator<'a, T>>,
    initial: bool,
}

impl<'a, T: CoordinateType + 'a> PointsOfInflectionIterator<'a, T> {
    fn advance_segment_iterators(&mut self, first: bool, second: bool) {
        debug_assert!(!self.initial || (first && second));
        if !self.initial {
            if first {
                self.first_segment_iterator.next();
            }
            if second {
                self.second_segment_iterator.next();
            }
        }
        self.initial = false;
    }
}

impl<'a, T: CoordinateType + 'a> Iterator for PointsOfInflectionIterator<'a, T> {
    type Item = (T, T, T);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.first.peek(), self.second.peek()) {
            (Some(first), Some(second)) => {
                if first.x == second.x {
                    let first = self.first.next().unwrap();
                    let second = self.second.next().unwrap();
                    self.advance_segment_iterators(true, true);
                    Some((first.x, first.y, second.y))
                } else if first.x < second.x {
                    let first = self.first.next().unwrap();
                    let y2 = y_at_x(self.second_segment_iterator.peek().unwrap(), first.x);
                    self.advance_segment_iterators(true, false);
                    Some((first.x, first.y, y2))
                } else {
                    let second = self.second.next().unwrap();
                    let y1 = y_at_x(self.first_segment_iterator.peek().unwrap(), second.x);
                    self.advance_segment_iterators(false, true);
                    Some((second.x, y1, second.y))
                }
            }
            (None, None) => None,
            (Some(_), None) | (None, Some(_)) => panic!(
                "domain constraints should guarantee that both segment iterators get exhausted at \
                 the same time"
            ),
        }
    }
}

/// Structure returned by [PiecewiseLinearFunction::segments_iter].
pub struct SegmentsIterator<'a, T: CoordinateType + 'a>(
    ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
);

impl<'a, T: CoordinateType + 'a> Iterator for SegmentsIterator<'a, T> {
    type Item = Line<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().and_then(|first| {
            self.0
                .peek()
                .map(|second| Line::new(first.clone(), *second.clone()))
        })
    }
}

/// Returns the restriction of segment `l` to the given domain, or `None` if the line's
/// intersection with the domain is either a singleton or empty.
pub fn line_in_domain<T: CoordinateType>(l: &Line<T>, domain: (T, T)) -> Option<Line<T>> {
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

fn x_at_y<T: CoordinateType>(line: &Line<T>, y: T) -> T {
    line.start.x + (y - line.start.y) / line.slope()
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
    fn test_x_at_y() {
        assert_eq!(x_at_y(&Line::new((0., 0.), (1., 1.)), 0.25), 0.25);
        assert_eq!(x_at_y(&Line::new((1., 0.), (2., 1.)), 0.25), 1.25);
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
        let f: PiecewiseLinearFunction<_> = vec![(0., 0.), (1., 1.), (2., 1.5)].try_into().unwrap();
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
        let f: PiecewiseLinearFunction<_> = vec![(0., 0.), (1., 1.), (2., 1.5)].try_into().unwrap();
        let g: PiecewiseLinearFunction<_> =
            vec![(0., 0.), (1.5, 3.), (2., 10.)].try_into().unwrap();
        for x in f.points_of_inflection_iter(&g).unwrap() {
            println!("{:?}", x);
        }
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
}
