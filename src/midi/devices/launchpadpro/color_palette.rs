use crate::midi::{Error, Event};
use crate::midi::features::{R, ColorPalette};

use super::device::LaunchpadProFeatures;

/// On the Launchpad Pro, we’ll use the bottom row to select colors:
///    ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮
///    ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
/// ╭╮ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╔╗ ╭╮
/// ╰╯ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╚╝ ╰╯
///    ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮ ╭╮
///    ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯ ╰╯
///     ↖0 ↖1 ↖2 ↖3 ↖4 ↖5 ↖6 ↖7
impl ColorPalette for LaunchpadProFeatures {
    fn into_color_palette_index(&self, event: Event) -> R<Option<usize>> {
        return Ok(match event {
            // 176: controller on
            // data1: between 1 and 8
            // data2: strictly positive (the key must be pressed)
            Event::Midi([176, data1, data2, _]) if data2 > 0 => {
                if data1 >= 1 && data1 <= 8 {
                    Some(data1 - 1).map(|index| index.into())
                } else {
                    None
                }
            },
            _ => None,
        });
    }

    fn from_color_palette(&self, colors: Vec<[u8; 3]>) -> R<Event> {
        if colors.len() > 8 {
            return Err(Box::new(Error::OutOfBoundIndexError));
        }

        let mut bytes = vec![240, 0, 32, 41, 2, 16, 11];

        for index in 0..colors.len() {
            let led = (index + 1) as u8;
            bytes.append(&mut vec![
                led,
                colors[index][0] / 4,
                colors[index][1] / 4,
                colors[index][2] / 4,
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
    fn into_color_palette_index_given_incorrect_status_should_return_none() {
        let features = super::super::LaunchpadProFeatures::new();
        let event = Event::Midi([128, 3, 10, 0]);
        assert_eq!(None, features
            .into_color_palette_index(event)
            .expect("into_color_palette_index should not fail"));
    }

    #[test]
    fn into_color_palette_index_given_low_velocity_should_return_none() {
        let features = super::super::LaunchpadProFeatures::new();
        let event = Event::Midi([176, 3, 0, 0]);
        assert_eq!(None, features
            .into_color_palette_index(event)
            .expect("into_color_palette_index should not fail"));
    }

    #[test]
    fn into_color_palette_index_given_out_of_grid_value_should_return_none() {
        let features = super::super::LaunchpadProFeatures::new();
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
            assert_eq!(None, features
                .into_color_palette_index(event)
                .expect("into_color_palette_index should not fail"));
        }
    }

    #[test]
    fn into_color_palette_index_should_correct_value() {
        let features = super::super::LaunchpadProFeatures::new();
        let actual_output = vec![1, 2, 3, 4, 5, 6, 7, 8]
            .iter()
            .map(|code| features
                .into_color_palette_index(Event::Midi([176, *code, 10, 0]))
                .expect("into_color_palette_index should not fail"))
            .collect::<Vec<Option<usize>>>();

        let expected_output = vec![0, 1, 2, 3, 4, 5, 6, 7]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<usize>>>();

        assert_eq!(expected_output, actual_output);
    }

    #[test]
    fn from_color_palette_when_too_many_colors_then_return_out_of_bound_error() {
        let features = super::super::LaunchpadProFeatures::new();
        // a color palette of nine items should not be supported (even if they’re all black)
        let color_palette = vec![[0, 0, 0]; 9];
        let actual_event = features.from_color_palette(color_palette);
        assert!(actual_event.is_err());
    }

    #[test]
    fn from_color_palette_when_valid_palette_then_divide_all_values_by_four() {
        let features = super::super::LaunchpadProFeatures::new();
        let color_palette = vec![
            [12, 24, 48],
            [96, 16, 36],
            [8, 192, 56],
        ];

        let actual_event = features.from_color_palette(color_palette).unwrap();
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
