use crate::{Database, Result, settings};

const WINDOW_KEY: &str = "window";
const MAXIMIZED: &str = "maximized";
const WINDOWED: &str = "windowed";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub area: Area,
    pub maximized: bool,
}

impl Area {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

impl Placement {
    pub fn key(&self) -> String {
        let state = if self.maximized { MAXIMIZED } else { WINDOWED };
        let Area {
            x,
            y,
            width,
            height,
        } = self.area;
        format!("{state} {x} {y} {width} {height}")
    }

    pub fn from_key(value: &str) -> Option<Self> {
        let mut parts = value.split_whitespace();
        let maximized = match parts.next()? {
            MAXIMIZED => true,
            WINDOWED => false,
            _ => return None,
        };
        let numbers: Vec<f32> = parts
            .map(|part| part.parse::<f32>().ok().filter(|number| number.is_finite()))
            .collect::<Option<_>>()?;
        let [x, y, width, height] = numbers[..] else {
            return None;
        };
        (width > 0.0 && height > 0.0).then_some(Self {
            area: Area {
                x,
                y,
                width,
                height,
            },
            maximized,
        })
    }

    pub fn display_among(&self, displays: &[Area]) -> Option<usize> {
        let centre_x = self.area.x + self.area.width / 2.0;
        let centre_y = self.area.y + self.area.height / 2.0;
        displays
            .iter()
            .position(|display| display.contains(centre_x, centre_y))
    }
}

pub fn placement(database: &Database) -> Result<Option<Placement>> {
    Ok(settings::get(database, WINDOW_KEY)?.and_then(|value| Placement::from_key(&value)))
}

pub fn set_placement(database: &Database, placement: Placement) -> Result<()> {
    settings::set(database, WINDOW_KEY, &placement.key())
}

#[cfg(test)]
mod tests {
    use super::{Area, Placement, placement, set_placement};
    use crate::Database;

    fn area(x: f32, y: f32, width: f32, height: f32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    fn windowed(x: f32, y: f32) -> Placement {
        Placement {
            area: area(x, y, 1280.0, 800.0),
            maximized: false,
        }
    }

    #[test]
    fn a_fresh_library_has_no_saved_window() {
        let database = Database::open_in_memory().unwrap();

        assert_eq!(placement(&database).unwrap(), None);
    }

    #[test]
    fn a_maximized_window_and_its_restore_size_round_trip() {
        let database = Database::open_in_memory().unwrap();
        let saved = Placement {
            area: area(-1700.5, 40.0, 1500.0, 900.25),
            maximized: true,
        };

        set_placement(&database, saved).unwrap();

        assert_eq!(placement(&database).unwrap(), Some(saved));
    }

    #[test]
    fn a_windowed_window_round_trips_and_replaces_the_last_one() {
        let database = Database::open_in_memory().unwrap();

        set_placement(
            &database,
            Placement {
                area: area(0.0, 0.0, 1000.0, 700.0),
                maximized: true,
            },
        )
        .unwrap();
        set_placement(&database, windowed(120.0, 60.0)).unwrap();

        assert_eq!(placement(&database).unwrap(), Some(windowed(120.0, 60.0)));
    }

    #[test]
    fn unreadable_saved_windows_are_ignored() {
        for value in [
            "",
            "maximized",
            "maximized 1 2 3",
            "maximized 1 2 3 4 5",
            "sideways 0 0 1280 800",
            "windowed 0 0 wide 800",
            "windowed 0 0 0 800",
            "windowed 0 0 1280 -800",
            "windowed NaN 0 1280 800",
            "windowed 0 0 inf 800",
        ] {
            assert_eq!(Placement::from_key(value), None, "{value:?}");
        }
    }

    #[test]
    fn a_window_reopens_on_the_display_that_holds_its_centre() {
        let displays = [
            area(0.0, 0.0, 1920.0, 1080.0),
            area(1920.0, 0.0, 2560.0, 1440.0),
        ];

        assert_eq!(windowed(100.0, 100.0).display_among(&displays), Some(0));
        assert_eq!(windowed(2400.0, 200.0).display_among(&displays), Some(1));
        assert_eq!(windowed(1500.0, 100.0).display_among(&displays), Some(1));
    }

    #[test]
    fn a_window_whose_display_is_gone_has_no_display() {
        let displays = [area(0.0, 0.0, 1920.0, 1080.0)];

        assert_eq!(windowed(2400.0, 200.0).display_among(&displays), None);
        assert_eq!(windowed(-2000.0, 100.0).display_among(&displays), None);
        assert_eq!(windowed(100.0, 100.0).display_among(&[]), None);
    }
}
