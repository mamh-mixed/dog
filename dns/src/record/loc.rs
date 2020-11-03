use std::fmt;

use log::*;

use crate::wire::*;


/// A **LOC** _(location)_ record, which points to a location on Earth using
/// its latitude, longitude, and altitude.
///
/// # References
///
/// - [RFC 1876](https://tools.ietf.org/html/rfc1876) — A Means for Expressing Location Information in the Domain Name System (January 1996)
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct LOC {

    /// The diameter of a sphere enclosing the entity at the location, as a
    /// measure of its size, measured in centimetres.
    pub size: Size,

    /// The diameter of the “circle of error” that this location could be in,
    /// measured in centimetres.
    pub horizontal_precision: u8,

    /// The amount of vertical space that this location could be in, measured
    /// in centimetres.
    pub vertical_precision: u8,

    /// The latitude of the centre of the sphere.
    pub latitude: Position,

    /// The longitude of the centre of the sphere.
    pub longitude: Position,

    /// The altitude of the centre of the sphere, measured in centimetres
    /// above a base of 100,000 metres below the GPS reference spheroid.
    pub altitude: u32,
}

/// A measure of size, in centimetres, represented by a base and an exponent.
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Size {
    base: u8,
    power_of_ten: u8,
}

/// A position on one of the world’s axes.
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Position {
    degrees: u32,
    arcminutes: u32,
    arcseconds: u32,
    milliarcseconds: u32,
    direction: Direction,
}

/// One of the directions a position could be in, relative to the equator or
/// prime meridian.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Wire for LOC {
    const NAME: &'static str = "LOC";
    const RR_TYPE: u16 = 29;

    #[cfg_attr(all(test, feature = "with_mutagen"), ::mutagen::mutate)]
    fn read(stated_length: u16, c: &mut Cursor<&[u8]>) -> Result<Self, WireError> {
        let version = c.read_u8()?;
        trace!("Parsed version -> {:?}", version);

        if version != 0 {
            return Err(WireError::WrongVersion {
                stated_version: version,
                maximum_supported_version: 0,
            });
        }

        if stated_length != 16 {
            let mandated_length = MandatedLength::Exactly(16);
            return Err(WireError::WrongRecordLength { stated_length, mandated_length });
        }

        let size_bits = c.read_u8()?;
        trace!("Parsed size bits -> {:#08b}", size_bits);

        let base = size_bits >> 4;
        let power_of_ten = size_bits & 0b_0000_1111;
        trace!("Split size into base {:?} and power of ten {:?}", base, power_of_ten);
        let size = Size { base, power_of_ten };

        let horizontal_precision = c.read_u8()?;
        trace!("Parsed horizontal precision -> {:?}", horizontal_precision);

        let vertical_precision = c.read_u8()?;
        trace!("Parsed vertical precision -> {:?}", vertical_precision);

        let latitude_num = c.read_u32::<BigEndian>()?;
        let latitude = Position::from_u32(latitude_num, true);
        trace!("Parsed latitude -> {:?} ({})", latitude_num, latitude);

        let longitude_num = c.read_u32::<BigEndian>()?;
        let longitude = Position::from_u32(longitude_num, false);
        trace!("Parsed longitude -> {:?} ({})", longitude_num, longitude);

        let altitude = c.read_u32::<BigEndian>()?;
        trace!("Parsed altitude -> {:?}", altitude);

        Ok(Self {
            size, horizontal_precision, vertical_precision, latitude, longitude, altitude,
        })
    }
}

impl Position {

    /// Converts a number into the position it represents. The input number is
    /// measured in thousandths of an arcsecond (milliarcseconds), with 2^31
    /// as the equator or prime meridian.
    fn from_u32(mut input: u32, vertical: bool) -> Self {
        if input >= 0x_8000_0000 {
            input -= 0x_8000_0000;
            let milliarcseconds = input % 1000;
            let total_arcseconds = input / 1000;

            let arcseconds = total_arcseconds % 60;
            let total_arcminutes = total_arcseconds / 60;

            let arcminutes = total_arcminutes % 60;
            let degrees = total_arcminutes / 60;

            let direction = if vertical { Direction::North }
                                   else { Direction::East };

            Self { degrees, arcminutes, arcseconds, milliarcseconds, direction }
        }
        else {
            let mut pos = Self::from_u32(input + (0x_8000_0000_u32 - input) * 2, vertical);

            pos.direction = if vertical { Direction::South }
                                   else { Direction::West };
            pos
        }
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}e{}", self.base, self.power_of_ten)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}°{}′{}",
            self.degrees,
            self.arcminutes,
            self.arcseconds,
        )?;

        if self.milliarcseconds != 0 {
            write!(f, ".{:03}", self.milliarcseconds)?;
        }

        write!(f, "″ {}", self.direction)
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::North  => write!(f, "N"),
            Self::East   => write!(f, "E"),
            Self::South  => write!(f, "S"),
            Self::West   => write!(f, "W"),
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses() {
        let buf = &[
            0x00,  // version
            0x32,  // size,
            0x00,  // horizontal precision
            0x00,  // vertical precision
            0x8b, 0x0d, 0x2c, 0x8c,  // latitude
            0x7f, 0xf8, 0xfc, 0xa5,  // longitude
            0x00, 0x98, 0x96, 0x80,  // altitude
        ];

        assert_eq!(LOC::read(buf.len() as _, &mut Cursor::new(buf)).unwrap(),
                   LOC {
                       size: Size { base: 3, power_of_ten: 2 },
                       horizontal_precision: 0,
                       vertical_precision: 0,
                       latitude:  Position::from_u32(0x_8b_0d_2c_8c, true),
                       longitude: Position::from_u32(0x_7f_f8_fc_a5, false),
                       altitude:  0x_00_98_96_80,
                   });
    }

    #[test]
    fn record_too_short() {
        let buf = &[
            0x00,  // version
            0x00,  // size
        ];

        assert_eq!(LOC::read(buf.len() as _, &mut Cursor::new(buf)),
                   Err(WireError::WrongRecordLength { stated_length: 2, mandated_length: MandatedLength::Exactly(16) }));
    }

    #[test]
    fn record_too_long() {
        let buf = &[
            0x00,  // version
            0x32,  // size,
            0x00,  // horizontal precision
            0x00,  // vertical precision
            0x8b, 0x0d, 0x2c, 0x8c,  // latitude
            0x7f, 0xf8, 0xfc, 0xa5,  // longitude
            0x00, 0x98, 0x96, 0x80,  // altitude
            0x12, 0x34, 0x56,  // some other stuff
        ];

        assert_eq!(LOC::read(buf.len() as _, &mut Cursor::new(buf)),
                   Err(WireError::WrongRecordLength { stated_length: 19, mandated_length: MandatedLength::Exactly(16) }));
    }

    #[test]
    fn more_recent_version() {
        let buf = &[
            0x80,  // version
            0x12, 0x34, 0x56,  // some data in an unknown format
        ];

        assert_eq!(LOC::read(buf.len() as _, &mut Cursor::new(buf)),
                   Err(WireError::WrongVersion { stated_version: 128, maximum_supported_version: 0 }));
    }

    #[test]
    fn record_empty() {
        assert_eq!(LOC::read(0, &mut Cursor::new(&[])),
                   Err(WireError::IO));
    }

    #[test]
    fn buffer_ends_abruptly() {
        let buf = &[
            0x00,  // version
        ];

        assert_eq!(LOC::read(16, &mut Cursor::new(buf)),
                   Err(WireError::IO));
    }
}


#[cfg(test)]
mod position_test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn meridian() {
        assert_eq!(Position::from_u32(0x_8000_0000, false).to_string(),
                   String::from("0°0′0″ E"));
    }

    #[test]
    fn meridian_plus_one() {
        assert_eq!(Position::from_u32(0x_8000_0000 + 1, false).to_string(),
                   String::from("0°0′0.001″ E"));
    }

    #[test]
    fn meridian_minus_one() {
        assert_eq!(Position::from_u32(0x_8000_0000 - 1, false).to_string(),
                   String::from("0°0′0.001″ W"));
    }

    #[test]
    fn equator() {
        assert_eq!(Position::from_u32(0x_8000_0000, true).to_string(),
                   String::from("0°0′0″ N"));
    }

    #[test]
    fn equator_plus_one() {
        assert_eq!(Position::from_u32(0x_8000_0000 + 1, true).to_string(),
                   String::from("0°0′0.001″ N"));
    }

    #[test]
    fn equator_minus_one() {
        assert_eq!(Position::from_u32(0x_8000_0000 - 1, true).to_string(),
                   String::from("0°0′0.001″ S"));
    }

    #[test]
    fn some_latitude() {
        assert_eq!(Position::from_u32(2332896396, true).to_string(),
                   String::from("51°30′12.748″ N"));
    }

    #[test]
    fn some_longitude() {
        assert_eq!(Position::from_u32(2147024037, false).to_string(),
                   String::from("0°7′39.611″ W"));
    }
}