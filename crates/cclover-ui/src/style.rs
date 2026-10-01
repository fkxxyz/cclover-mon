#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Background,
    Card,
    Border,
    Foreground,
    Muted,
    Accent,
    Green,
    Orange,
    Red,
    Guide,
}

impl Tone {
    pub const fn rgba(self) -> Rgba {
        match self {
            Self::Background => Rgba {
                r: 0x0b,
                g: 0x10,
                b: 0x20,
                a: 0.85,
            },
            Self::Card => Rgba {
                r: 0x15,
                g: 0x1b,
                b: 0x2d,
                a: 0.80,
            },
            Self::Border => Rgba {
                r: 0x33,
                g: 0x41,
                b: 0x5f,
                a: 1.0,
            },
            Self::Foreground => Rgba {
                r: 0xf3,
                g: 0xf6,
                b: 0xff,
                a: 1.0,
            },
            Self::Muted => Rgba {
                r: 0x8f,
                g: 0x9b,
                b: 0xb0,
                a: 1.0,
            },
            Self::Accent => Rgba {
                r: 0x7c,
                g: 0x9c,
                b: 0xff,
                a: 1.0,
            },
            Self::Green => Rgba {
                r: 0x52,
                g: 0xe0,
                b: 0xc4,
                a: 1.0,
            },
            Self::Orange => Rgba {
                r: 0xff,
                g: 0xb8,
                b: 0x6b,
                a: 1.0,
            },
            Self::Red => Rgba {
                r: 0xff,
                g: 0x7e,
                b: 0x9b,
                a: 1.0,
            },
            Self::Guide => Rgba {
                r: 0x26,
                g: 0x34,
                b: 0x4e,
                a: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWeight {
    Regular,
    Bold,
}
