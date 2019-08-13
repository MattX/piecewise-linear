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

use geo::{Coordinate, CoordinateType, Line, Point};

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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
