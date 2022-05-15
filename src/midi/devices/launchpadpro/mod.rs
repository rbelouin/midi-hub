mod device;

mod app_selector;
mod color_palette;
mod grid_controller;
mod image_renderer;
mod index_selector;

pub use device::LaunchpadPro;
pub use device::transformer;

#[cfg(test)]
mod test {
    #[test]
    #[cfg(feature = "launchpadpro")]
    fn render_rainbow_and_blink() {
        use std::convert::From;
        use crate::image::Image;
        use crate::midi::{Connections, FromImage, FromSelectedIndex, Writer};
        use super::*;

        let connections = Connections::new().unwrap();
        let ports = connections.create_bidirectional_ports(&"Launchpad Pro Standalone Port".to_string());
        match ports {
            Ok(ports) => {
                let mut launchpadpro = LaunchpadPro::from(ports);
                let mut bytes = vec![0u8; 192];

                for y in 0..8 {
                    for x in 0..8 {
                        let index = x + y;
                        bytes[3 * (y * 8 + x) + 0] = (255 - 255 * index / 14) as u8;
                        bytes[3 * (y * 8 + x) + 1] = 0;
                        bytes[3 * (y * 8 + x) + 2] = (255 * index / 14) as u8;
                    }
                }

                let image = Image {
                    width: 8,
                    height: 8,
                    bytes,
                };

                let result = LaunchpadProEvent::from_image(image).and_then(|event| {
                    return launchpadpro.write(event);
                });

                assert!(result.is_ok(), "The LaunchpadPro could not render the given image");

                let result = LaunchpadProEvent::from_selected_index(27).and_then(|event| {
                    return launchpadpro.write(event);
                });

                assert!(result.is_ok(), "The LaunchpadPro could not make the square pad blink");
            },
            Err(_) => {
                println!("The LaunchpadPro device may not be connected correctly");
            }
        }
    }
}
