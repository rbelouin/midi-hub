use crate::midi::Event;
use crate::midi::features::{R, GridController};

use super::device::LaunchpadProFeatures;

impl GridController for LaunchpadProFeatures {
    fn get_grid_size(&self) -> R<(usize, usize)> {
        return Ok((8, 8));
    }

    fn into_coordinates(&self, event: Event) -> R<Option<(usize, usize)>> {
        return Ok(match event {
            // event must be a "note down" (144) with a strictly positive velocity
            Event::Midi([144, data1, data2, _]) if data2 > 0 => {
                // the device provides a 10x10 grid if you count the buttons on the sides
                let row = data1 / 10;
                let column  = data1 % 10;

                // we’ll only return coordinates for the central 8x8 grid
                if row >= 1 && row <= 8 && column >= 1 && column <= 8 {
                    Some(((column - 1).into(), (8 - row).into()))
                } else {
                    None
                }
            },
            _ => None,
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn into_coordinates_given_incorrect_status_should_return_none() {
        let features = super::super::LaunchpadProFeatures::new();
        let event = Event::Midi([128, 53, 10, 0]);
        assert_eq!(None, features.into_coordinates(event).expect("into_coordinates should not fail"));
    }

    #[test]
    fn into_coordinates_given_low_velocity_should_return_none() {
        let features = super::super::LaunchpadProFeatures::new();
        let event = Event::Midi([144, 53, 0, 0]);
        assert_eq!(None, features.into_coordinates(event).expect("into_coordinates should not fail"));
    }

    #[test]
    fn into_coordinates_given_out_of_grid_value_should_return_none() {
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
            let features = super::super::LaunchpadProFeatures::new();
            let event = Event::Midi(event);
            assert_eq!(None, features.into_coordinates(event).expect("into_coordinates should not fail"));
        }
    }

    #[test]
    fn into_coordinates_should_correct_value() {
        let features = super::super::LaunchpadProFeatures::new();
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
            .map(|code| features
                .into_coordinates(Event::Midi([144, *code, 10, 0]))
                .expect("into_coordinates should not fail"))
            .collect::<Vec<Option<(usize, usize)>>>();

        let expected_output = vec![
            (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0),
            (0, 1), (1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (6, 1), (7, 1),
            (0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2), (6, 2), (7, 2),
            (0, 3), (1, 3), (2, 3), (3, 3), (4, 3), (5, 3), (6, 3), (7, 3),
            (0, 4), (1, 4), (2, 4), (3, 4), (4, 4), (5, 4), (6, 4), (7, 4),
            (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5), (6, 5), (7, 5),
            (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6), (7, 6),
            (0, 7), (1, 7), (2, 7), (3, 7), (4, 7), (5, 7), (6, 7), (7, 7),
        ]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<(usize, usize)>>>();

        assert_eq!(expected_output, actual_output);
    }
}
