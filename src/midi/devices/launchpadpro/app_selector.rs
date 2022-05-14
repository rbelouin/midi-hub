use crate::midi::{Error, Event};
use crate::midi::features::{R, AppSelector};

use super::device::LaunchpadProEventTransformer;

/// On the Launchpad Pro, we’ll use the right column to select applications:
///    ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮
///    ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 0
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 1
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 2
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 3
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 4
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 5
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 6
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯ ↖ App 7
///    ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮
///    ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯

impl AppSelector for LaunchpadProEventTransformer {
    fn into_app_index(&self, event: Event) ->  R<Option<usize>> {
        return Ok(match event {
            // event must be a "note down" with a strictly positive velocity
            // 176: controller on
            // data1: 19/29/../89
            // data2: strictly positive (the key must be pressed)
            Event::Midi([176, data1, data2, _]) if data2 > 0 => {
                // the device provides a 10x10 grid if you count the buttons on the sides
                let row = data1 / 10;
                let column  = data1 % 10;

                if row >= 1 && row <= 8 && column == 9 {
                    Some(8 - row).map(|index| index.into())
                } else {
                    None
                }
            },
            _ => None,
        });
    }

    fn from_app_colors(&self, app_colors: Vec<[u8; 3]>) -> R<Event> {
        if app_colors.len() > 8 {
            return Err(Box::new(Error::OutOfBoundIndexError));
        }

        let mut bytes = vec![240, 0, 32, 41, 2, 16, 11];

        for index in 0..app_colors.len() {
            let led = (89 - 10 * index) as u8;
            bytes.append(&mut vec![
                led,
                app_colors[index][0] / 4,
                app_colors[index][1] / 4,
                app_colors[index][2] / 4,
            ]);
        }
        bytes.push(247);

        return Ok(Event::SysEx(bytes));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn into_app_index_given_incorrect_status_should_return_none() {
        let transformer = super::super::transformer();
        let event = Event::Midi([128, 89, 10, 0]);
        assert_eq!(None, transformer.into_app_index(event).expect("into_app_index should not fail"));
    }

    #[test]
    fn into_app_index_given_low_velocity_should_return_none() {
        let transformer = super::super::transformer();
        let event = Event::Midi([176, 89, 0, 0]);
        assert_eq!(None, transformer.into_app_index(event).expect("into_app_index should not fail"));
    }

    #[test]
    fn into_app_index_given_out_of_grid_value_should_return_none() {
        let transformer = super::super::transformer();
        let events = vec![
            [176, 08, 10, 0],
            [176, 09, 10, 0],
            [176, 18, 10, 0],
            [176, 28, 10, 0],
            [176, 38, 10, 0],
            [176, 48, 10, 0],
            [176, 58, 10, 0],
            [176, 68, 10, 0],
            [176, 78, 10, 0],
            [176, 88, 10, 0],
            [176, 98, 10, 0],
            [176, 99, 10, 0],
        ];

        for event in events {
            let event = Event::Midi(event);
            assert_eq!(None, transformer.into_app_index(event).expect("into_app_index should not fail"));
        }
    }

    #[test]
    fn into_app_index_should_correct_value() {
        let transformer = super::super::transformer();
        let actual_output = vec![19, 29, 39, 49, 59, 69, 79, 89]
            .iter()
            .map(|code| transformer
                .into_app_index(Event::Midi([176, *code, 10, 0]))
                .expect("into_app_index should not fail"))
            .collect::<Vec<Option<usize>>>();

        let expected_output = vec![7, 6, 5, 4, 3, 2, 1, 0]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<usize>>>();

        assert_eq!(expected_output, actual_output);
    }

    #[test]
    fn from_app_colors_when_too_many_colors_then_return_out_of_bound_error() {
        let transformer = super::super::transformer();
        // the Launchpad Pro won’t support nine applications, even if they all use black!
        let app_colors = vec![[0, 0, 0]; 9];
        let actual_event = transformer.from_app_colors(app_colors);
        assert!(actual_event.is_err());
    }

    #[test]
    fn from_app_colors_when_valid_apps_then_divide_all_values_by_four() {
        let transformer = super::super::transformer();
        let app_colors = vec![
            [12, 24, 48],
            [96, 16, 36],
            [8, 192, 56],
        ];

        let actual_event = transformer.from_app_colors(app_colors).unwrap();
        assert_eq!(actual_event, Event::SysEx(vec![
                // Prefix for "bluk lighting" a set of LEDs
                240, 0, 32, 41, 2, 16, 11,
                // Identifier for the first LED
                89,
                // The Launchpad Pro only accepts 3-byte colors,
                // where each byte has a value within the [0; 63] range.
                3, 6, 12,
                // Identifier and color for the second LED
                79, 24, 4, 9,
                // Identifier and color for the third LED
                69, 2, 48, 14,
                // Suffix for LaunchpadPro SysEx commands
                247,
        ]));
    }
}
