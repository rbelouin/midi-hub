use crate::midi::{Event, Error};

pub fn into_index(event: Event) ->  Result<Option<u16>, Error> {
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

pub fn into_color_palette_index(event: Event) ->  Result<Option<u16>, Error> {
    return Ok(match event {
        // event must be a "note down" with a strictly positive velocity
        Event::Midi([176, data1, data2, _]) if data2 > 0 => {
            // the device provides a 10x10 grid if you count the buttons on the sides
            let row = data1 / 10;
            let column  = data1 % 10;

            // we’ll use the bottom row to select colors
            if row == 0 && column >= 1 && column <= 8 {
                Some(column - 1).map(|index| index.into())
            } else {
                None
            }
        },
        _ => None,
    });
}

pub fn from_index_to_highlight(index: u16) -> Result<Event, Error> {
    if index > 63 {
        return Err(Error::OutOfBoundIndexError);
    }

    let index = index as u8;
    let row = index / 8 + 1;
    let column = index % 8 + 1;
    let led = row * 10 + column;

    let bytes = vec![240, 0, 32, 41, 2, 16, 40, led, 45, 247];
    return Ok(Event::SysEx(bytes));
}

pub fn from_color_palette(color_palette: Vec<[u8; 3]>) -> Result<Event, Error> {
    if color_palette.len() > 8 {
        return Err(Error::OutOfBoundIndexError);
    }

    let mut bytes = vec![240, 0, 32, 41, 2, 16, 11];

    for index in 0..color_palette.len() {
        let led = (index + 1) as u8;
        bytes.append(&mut vec![
            led,
            color_palette[index][0] / 4,
            color_palette[index][1] / 4,
            color_palette[index][2] / 4,
        ]);
    }
    bytes.push(247);

    return Ok(Event::SysEx(bytes));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_index_given_incorrect_status_should_return_none() {
        let event = Event::Midi([128, 53, 10, 0]);
        assert_eq!(None, into_index(event).expect("into_index should not fail"));
    }

    #[test]
    fn into_index_given_low_velocity_should_return_none() {
        let event = Event::Midi([144, 53, 0, 0]);
        assert_eq!(None, into_index(event).expect("into_index should not fail"));
    }

    #[test]
    fn into_index_given_out_of_grid_value_should_return_none() {
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
            assert_eq!(None, into_index(event).expect("into_index should not fail"));
        }
    }

    #[test]
    fn into_index_should_correct_value() {
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
            .map(|code| into_index(Event::Midi([144, *code, 10, 0])).expect("into_index should not fail"))
            .collect::<Vec<Option<u16>>>();

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
            .collect::<Vec<Option<u16>>>();

        assert_eq!(expected_output, actual_output);
    }

    #[test]
    fn into_color_palette_index_given_incorrect_status_should_return_none() {
        let event = Event::Midi([128, 3, 10, 0]);
        assert_eq!(None, into_color_palette_index(event).expect("into_color_palette_index should not fail"));
    }

    #[test]
    fn into_color_palette_index_given_low_velocity_should_return_none() {
        let event = Event::Midi([176, 3, 0, 0]);
        assert_eq!(None, into_color_palette_index(event).expect("into_color_palette_index should not fail"));
    }

    #[test]
    fn into_color_palette_index_given_out_of_grid_value_should_return_none() {
        let events = vec![
            [176, 00, 10, 0],
            [176, 09, 10, 0],
            [176, 10, 10, 0],
            [176, 11, 10, 0],
            [176, 12, 10, 0],
            [176, 13, 10, 0],
            [176, 14, 10, 0],
            [176, 15, 10, 0],
            [176, 16, 10, 0],
            [176, 17, 10, 0],
            [176, 18, 10, 0],
            [176, 19, 10, 0],
        ];

        for event in events {
            let event = Event::Midi(event);
            assert_eq!(None, into_color_palette_index(event).expect("into_color_palette_index should not fail"));
        }
    }

    #[test]
    fn into_color_palette_index_should_correct_value() {
        let actual_output = vec![1, 2, 3, 4, 5, 6, 7, 8]
            .iter()
            .map(|code| into_color_palette_index(Event::Midi([176, *code, 10, 0])).expect("into_color_palette_index should not fail"))
            .collect::<Vec<Option<u16>>>();

        let expected_output = vec![0, 1, 2, 3, 4, 5, 6, 7]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<u16>>>();

        assert_eq!(expected_output, actual_output);
    }
// 2. values are divided by 4

    #[test]
    fn from_color_palette_when_too_many_colors_then_return_out_of_bound_error() {
        // a color palette of nine items should not be supported (even if they’re all black)
        let color_palette = vec![[0, 0, 0]; 9];
        let actual_event = from_color_palette(color_palette);
        assert_eq!(actual_event, Err(Error::OutOfBoundIndexError));
    }

    #[test]
    fn from_color_palette_when_valid_palette_then_divide_all_values_by_four() {
        let color_palette = vec![
            [12, 24, 48],
            [96, 16, 36],
            [8, 192, 56],
        ];

        let actual_event = from_color_palette(color_palette).unwrap();
        assert_eq!(actual_event, Event::SysEx(vec![
                // Prefix for "bluk lighting" a set of LEDs
                240, 0, 32, 41, 2, 16, 11,
                // Identifier for the first LED
                1,
                // The Launchpad Pro only accepts 3-byte colors,
                // where each byte has a value within the [0; 63] range.
                3, 6, 12,
                // Identifier and color for the second LED
                2, 24, 4, 9,
                // Identifier and color for the third LED
                3, 2, 48, 14,
                // Suffix for LaunchpadPro SysEx commands
                247,
        ]));
    }
}
