use crate::midi::{Event, Error, IntoIndex, IntoAppIndex, FromSelectedIndex, FromAppColors};
use super::LaunchpadProEvent;

impl IntoIndex for LaunchpadProEvent {
    fn into_index(self) ->  Result<Option<u16>, Error> {
        return Ok(match self.event {
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
}

impl IntoAppIndex for LaunchpadProEvent {
    fn into_app_index(self) ->  Result<Option<u16>, Error> {
        return Ok(match self.event {
            // event must be a "note down" with a strictly positive velocity
            Event::Midi([176, data1, data2, _]) if data2 > 0 => {
                // the device provides a 10x10 grid if you count the buttons on the sides
                let row = data1 / 10;
                let column  = data1 % 10;

                // we’ll use the last column on the right to select applications
                if row >= 1 && row <= 8 && column == 9 {
                    Some(8 - row).map(|index| index.into())
                } else {
                    None
                }
            },
            _ => None,
        });
    }
}

impl FromSelectedIndex<LaunchpadProEvent> for LaunchpadProEvent {
    fn from_selected_index(index: u16) -> Result<LaunchpadProEvent, Error> {
        if index > 63 {
            return Err(Error::OutOfBoundIndexError);
        }

        let index = index as u8;
        let row = index / 8 + 1;
        let column = index % 8 + 1;
        let led = row * 10 + column;

        let bytes = vec![240, 0, 32, 41, 2, 16, 40, led, 45, 247];
        return Ok(LaunchpadProEvent::from(Event::SysEx(bytes)));
    }
}

impl FromAppColors<LaunchpadProEvent> for LaunchpadProEvent {
    fn from_app_colors(app_colors: Vec<[u8; 3]>) -> Result<LaunchpadProEvent, Error> {
        if app_colors.len() > 8 {
            return Err(Error::OutOfBoundIndexError);
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

        return Ok(LaunchpadProEvent::from(Event::SysEx(bytes)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn into_index_given_incorrect_status_should_return_none() {
        let event = LaunchpadProEvent { event: Event::Midi([128, 53, 10, 0]) };
        assert_eq!(None, event.into_index().expect("into_index should not fail"));
    }

    #[test]
    fn into_index_given_low_velocity_should_return_none() {
        let event = LaunchpadProEvent { event: Event::Midi([144, 53, 0, 0]) };
        assert_eq!(None, event.into_index().expect("into_index should not fail"));
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
            let launchpadpro_event = LaunchpadProEvent { event: Event::Midi(event) };
            assert_eq!(None, launchpadpro_event.into_index().expect("into_index should not fail"));
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
            .map(|code| (LaunchpadProEvent { event: Event::Midi([144, *code, 10, 0]) }).into_index().expect("into_index should not fail"))
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
    fn into_app_index_given_incorrect_status_should_return_none() {
        let event = LaunchpadProEvent { event: Event::Midi([128, 89, 10, 0]) };
        assert_eq!(None, event.into_app_index().expect("into_app_index should not fail"));
    }

    #[test]
    fn into_app_index_given_low_velocity_should_return_none() {
        let event = LaunchpadProEvent { event: Event::Midi([176, 89, 0, 0]) };
        assert_eq!(None, event.into_app_index().expect("into_app_index should not fail"));
    }

    #[test]
    fn into_app_index_given_out_of_grid_value_should_return_none() {
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
            let launchpadpro_event = LaunchpadProEvent { event: Event::Midi(event) };
            assert_eq!(None, launchpadpro_event.into_app_index().expect("into_app_index should not fail"));
        }
    }

    #[test]
    fn into_app_index_should_correct_value() {
        let actual_output = vec![19, 29, 39, 49, 59, 69, 79, 89]
            .iter()
            .map(|code| (LaunchpadProEvent { event: Event::Midi([176, *code, 10, 0]) }).into_app_index().expect("into_app_index should not fail"))
            .collect::<Vec<Option<u16>>>();

        let expected_output = vec![7, 6, 5, 4, 3, 2, 1, 0]
            .iter()
            .map(|index| Some(*index))
            .collect::<Vec<Option<u16>>>();

        assert_eq!(expected_output, actual_output);
    }
}
