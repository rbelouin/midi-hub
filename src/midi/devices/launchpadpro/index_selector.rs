use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use crate::midi::Event;
use crate::midi::features::{R, IndexSelector};

use super::device::LaunchpadProEventTransformer;

#[derive(Debug)]
struct IndexOutOfBoundError {
    actual_value: usize,
    maximum_value: usize,
}

impl StdError for IndexOutOfBoundError {}
impl Display for IndexOutOfBoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "expected index with value below {}; got: {}", self.maximum_value, self.actual_value)
    }
}

impl IndexSelector for LaunchpadProEventTransformer {
    fn into_index(&self, event: Event) -> R<Option<usize>> {
        return Ok(match event {
            // event must be a "note down" with a strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data2 > 0 => {
                // the device provides a 10x10 grid if you count the buttons on the sides
                let row = data1 / 10;
                let column  = data1 % 10;

                // but in this implementation, we’ll only focus on the central 8x8 grid
                if row >= 1 && row <= 8 && column >= 1 && column <= 8 {
                    Some((row - 1) * 8 + (column - 1)).map(|index| index.into())
                } else {
                    None
                }
            },
            _ => None,
        });
    }

    fn from_index_to_highlight(&self, index: usize) -> R<Event> {
        if index > 63 {
            return Err(Box::new(IndexOutOfBoundError { actual_value: index, maximum_value: 63 }));
        }

        let index = index as u8;
        let row = index / 8 + 1;
        let column = index % 8 + 1;
        let led = row * 10 + column;

        let bytes = vec![240, 0, 32, 41, 2, 16, 40, led, 45, 247];
        return Ok(Event::SysEx(bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_index_given_incorrect_status_should_return_none() {
        let transformer = super::super::transformer();
        let event = Event::Midi([128, 53, 10, 0]);
        assert_eq!(None, transformer.into_index(event).expect("into_index should not fail"));
    }

    #[test]
    fn into_index_given_low_velocity_should_return_none() {
        let transformer = super::super::transformer();
        let event = Event::Midi([144, 53, 0, 0]);
        assert_eq!(None, transformer.into_index(event).expect("into_index should not fail"));
    }

    #[test]
    fn into_index_given_out_of_grid_value_should_return_none() {
        let transformer = super::super::transformer();
        let events = vec![
            [144, 00, 10, 0],
            [144, 01, 10, 0],
            [144, 08, 10, 0],
            [144, 08, 10, 0],
            [144, 10, 10, 0],
            [144, 19, 10, 0],
            [144, 80, 10, 0],
            [144, 89, 10, 0],
            [144, 90, 10, 0],
            [144, 91, 10, 0],
            [144, 98, 10, 0],
            [144, 99, 10, 0],
        ];

        for event in events {
            let event = Event::Midi(event);
            assert_eq!(None, transformer.into_index(event).expect("into_index should not fail"));
        }
    }

    #[test]
    fn into_index_should_correct_value() {
        let transformer = super::super::transformer();
        let actual_output = vec![
            81, 82, 83, 84, 85, 86, 87, 88,
            71, 72, 73, 74, 75, 76, 77, 78,
            61, 62, 63, 64, 65, 66, 67, 68,
            51, 52, 53, 54, 55, 56, 57, 58,
            41, 42, 43, 44, 45, 46, 47, 48,
            31, 32, 33, 34, 35, 36, 37, 38,
            21, 22, 23, 24, 25, 26, 27, 28,
            11, 12, 13, 14, 15, 16, 17, 18,
        ]
            .iter()
            .map(|code| transformer
                .into_index(Event::Midi([144, *code, 10, 0]))
                .expect("into_index should not fail"))
            .collect::<Vec<Option<usize>>>();

        let expected_output = vec![
            56, 57, 58, 59, 60, 61, 62, 63,
            48, 49, 50, 51, 52, 53, 54, 55,
            40, 41, 42, 43, 44, 45, 46, 47,
            32, 33, 34, 35, 36, 37, 38, 39,
            24, 25, 26, 27, 28, 29, 30, 31,
            16, 17, 18, 19, 20, 21, 22, 23,
            08, 09, 10, 11, 12, 13, 14, 15,
            00, 01, 02, 03, 04, 05, 06, 07,
        ]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<usize>>>();

        assert_eq!(expected_output, actual_output);
    }
}
