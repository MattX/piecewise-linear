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
use std::convert::TryFrom;

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
/// Example:
///
/// ```
/// use std::convert::TryInto;
/// use piecewise_linear::PiecewiseLinearFunction;
/// let f: PiecewiseLinearFunction<_> = vec![(0., 0.), (1., 1.), (2., 1.5)].try_into().unwrap();
/// assert_eq!(f.y_at_x(1.25).unwrap(), 1.125);
/// ```
#[derive(PartialEq, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PiecewiseLinearFunction<T: CoordinateType> {
    pub coordinates: Vec<Coordinate<T>>,
}

impl<T: CoordinateType> PiecewiseLinearFunction<T> {
    pub fn new(coordinates: Vec<Coordinate<T>>) -> Option<Self> {
        if coordinates.len() >= 2 && coordinates.windows(2).all(|w| w[0].x < w[1].x) {
            Some(PiecewiseLinearFunction { coordinates })
        } else {
            None
        }
    }

    pub fn points_of_inflection_iter<'a>(
        &'a self,
        other: &'a PiecewiseLinearFunction<T>,
    ) -> PointsOfInflectionIterator<T> {
        PointsOfInflectionIterator {
            first: self.coordinates.iter().peekable(),
            second: other.coordinates.iter().peekable(),
        }
    }

    pub fn segments_iter(&self) -> SegmentsIterator<T> {
        SegmentsIterator(self.coordinates.iter().peekable())
    }

    pub fn segment_at_x(&self, x: T) -> Option<Line<T>> {
        let idx = match self
            .coordinates
            .binary_search_by(|val| val.x.partial_cmp(&x).unwrap_or(Ordering::Equal))
        {
            Ok(idx) => idx,
            Err(idx) => {
                if idx == 0 || idx == self.coordinates.len() {
                    // Outside the function's definition set
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

    pub fn y_at_x(&self, x: T) -> Option<T> {
        self.segment_at_x(x).map(|line| y_at_x(&line, x))
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

// TODO rename
pub struct PointsOfInflectionIterator<'a, T: CoordinateType + 'a> {
    pub first: ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
    pub second: ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
}

impl<'a, T: CoordinateType + 'a> Iterator for PointsOfInflectionIterator<'a, T> {
    type Item = Point<T>;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.first.peek(), self.second.peek()) {
            (Some(first), Some(second)) => {
                if first.x == second.x {
                    self.first.next();
                    self.second.next()
                } else if first.x < second.x {
                    self.first.next()
                } else {
                    self.second.next()
                }
            }
            (Some(_), None) => self.first.next(),
            (None, Some(_)) => self.second.next(),
            (None, None) => None,
        }
        .map(|v| v.clone().into())
    }
}

pub struct SegmentsIterator<'a, T: CoordinateType + 'a>(
    pub ::std::iter::Peekable<::std::slice::Iter<'a, Coordinate<T>>>,
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

fn y_at_x<T: CoordinateType>(line: &Line<T>, x: T) -> T {
    line.start.y + (x - line.start.x) * line.slope()
}

fn x_at_y<T: CoordinateType>(line: &Line<T>, y: T) -> T {
    line.start.x + (y - line.start.y) / line.slope()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
