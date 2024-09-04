use palette::{Hsv, IntoColor, Srgb};

/*
Colors are hex strings that have easy access to their hue, saturation, value, and lightness.
Valid Color Strings Include:
- #RRGGBB
- #RGB
- #RRGGBBAA
- #RGBA

When color is modified, all properties are updated.
e.g if hsv/l is modified rgb are updated
if rgb is modified hsvl are updated
if v/l is modified the other is updated
*/

#[derive(Debug, Default)]
struct Color {
    alpha: i16,
    red: i16,
    green: i16,
    blue: i16,
    hue: i16,
    saturation: i16,
    value: i16,
    hex: String,
}

const MAX_RGB: i16 = 256;
const MAX_SVA: i16 = 100;
const MAX_HUE: i16 = 360;

#[derive(Debug)]
enum ColorError {
    InvalidHex(String),
    InvalidColorSettings,
}

#[derive(Debug)]
enum ColorChanges {
    AddHue(i16),
    AddSaturation(i16),
    AddValue(i16),
    AddRed(i16),
    AddGreen(i16),
    AddBlue(i16),
    AddAlpha(i16),

    MultHue(i16),
    MultSaturation(i16),
    MultValue(i16),
    MultRed(i16),
    MultGreen(i16),
    MultBlue(i16),
    MultAlpha(i16),

    DivHue(i16),
    DivSaturation(i16),
    DivValue(i16),
    DivRed(i16),
    DivGreen(i16),
    DivBlue(i16),
    DivAlpha(i16),
}

impl ColorChanges {
    fn from_setting(op: &str, b_val: &ColorSettings) -> Self {
        match op {
            "+" => match b_val {
                ColorSettings::SetHue(val) => Self::AddHue(*val),
                ColorSettings::SetSaturation(val) => Self::AddSaturation(*val),
                ColorSettings::SetValue(val) => Self::AddValue(*val),
                ColorSettings::SetRed(val) => Self::AddRed(*val),
                ColorSettings::SetGreen(val) => Self::AddGreen(*val),
                ColorSettings::SetBlue(val) => Self::AddBlue(*val),
                ColorSettings::SetAlpha(val) => Self::AddAlpha(*val),
            },

            "-" => match b_val {
                ColorSettings::SetHue(val) => Self::AddHue(-*val),
                ColorSettings::SetSaturation(val) => Self::AddSaturation(-*val),
                ColorSettings::SetValue(val) => Self::AddValue(-*val),
                ColorSettings::SetRed(val) => Self::AddRed(-*val),
                ColorSettings::SetGreen(val) => Self::AddGreen(-*val),
                ColorSettings::SetBlue(val) => Self::AddBlue(-*val),
                ColorSettings::SetAlpha(val) => Self::AddAlpha(-*val),
            },

            "*" => match b_val {
                ColorSettings::SetHue(val) => Self::MultHue(*val),
                ColorSettings::SetSaturation(val) => Self::MultSaturation(*val),
                ColorSettings::SetValue(val) => Self::MultValue(*val),
                ColorSettings::SetRed(val) => Self::MultRed(*val),
                ColorSettings::SetGreen(val) => Self::MultGreen(*val),
                ColorSettings::SetBlue(val) => Self::MultBlue(*val),
                ColorSettings::SetAlpha(val) => Self::MultAlpha(*val),
            },

            "/" => match b_val {
                ColorSettings::SetHue(val) => Self::DivHue(*val),
                ColorSettings::SetSaturation(val) => Self::DivSaturation(*val),
                ColorSettings::SetValue(val) => Self::DivValue(*val),
                ColorSettings::SetRed(val) => Self::DivRed(*val),
                ColorSettings::SetGreen(val) => Self::DivGreen(*val),
                ColorSettings::SetBlue(val) => Self::DivBlue(*val),
                ColorSettings::SetAlpha(val) => Self::DivAlpha(*val),
            },

            _ => panic!("Invalid operator"),
        }
    }

    fn apply_change(self, color: &Color) -> ColorSettings {
        match self {
            Self::AddHue(val) => ColorSettings::SetHue(color.hue + val),
            Self::AddSaturation(val) => ColorSettings::SetSaturation(color.saturation + val),
            Self::AddValue(val) => ColorSettings::SetValue(color.value + val),
            Self::AddRed(val) => ColorSettings::SetRed(color.red + val),
            Self::AddGreen(val) => ColorSettings::SetGreen(color.green + val),
            Self::AddBlue(val) => ColorSettings::SetBlue(color.blue + val),
            Self::AddAlpha(val) => ColorSettings::SetAlpha(color.alpha + val),

            Self::MultHue(val) => ColorSettings::SetHue(color.hue * val),
            Self::MultSaturation(val) => ColorSettings::SetSaturation(color.saturation * val),
            Self::MultValue(val) => ColorSettings::SetValue(color.value * val),
            Self::MultRed(val) => ColorSettings::SetRed(color.red * val),
            Self::MultGreen(val) => ColorSettings::SetGreen(color.green * val),
            Self::MultBlue(val) => ColorSettings::SetBlue(color.blue * val),
            Self::MultAlpha(val) => ColorSettings::SetAlpha(color.alpha * val),

            Self::DivHue(val) => ColorSettings::SetHue(color.hue / val),
            Self::DivSaturation(val) => ColorSettings::SetSaturation(color.saturation / val),
            Self::DivValue(val) => ColorSettings::SetValue(color.value / val),
            Self::DivRed(val) => ColorSettings::SetRed(color.red / val),
            Self::DivGreen(val) => ColorSettings::SetGreen(color.green / val),
            Self::DivBlue(val) => ColorSettings::SetBlue(color.blue / val),
            Self::DivAlpha(val) => ColorSettings::SetAlpha(color.alpha / val),
        }
    }

    fn apply_to_setting(op: &str, color: &Color, setting: &ColorSettings) -> ColorSettings {
        match op {
            "+" => match setting {
                ColorSettings::SetHue(val) => ColorSettings::SetHue(color.hue + val),
                ColorSettings::SetSaturation(val) => {
                    ColorSettings::SetSaturation(color.saturation + val)
                }
                ColorSettings::SetValue(val) => ColorSettings::SetValue(color.value + val),
                ColorSettings::SetRed(val) => ColorSettings::SetRed(color.red + val),
                ColorSettings::SetGreen(val) => ColorSettings::SetGreen(color.green + val),
                ColorSettings::SetBlue(val) => ColorSettings::SetBlue(color.blue + val),
                ColorSettings::SetAlpha(val) => ColorSettings::SetAlpha(color.alpha + val),
            },

            "-" => match setting {
                ColorSettings::SetHue(val) => ColorSettings::SetHue(color.hue - val),
                ColorSettings::SetSaturation(val) => {
                    ColorSettings::SetSaturation(color.saturation - val)
                }
                ColorSettings::SetValue(val) => ColorSettings::SetValue(color.value - val),
                ColorSettings::SetRed(val) => ColorSettings::SetRed(color.red - val),
                ColorSettings::SetGreen(val) => ColorSettings::SetGreen(color.green - val),
                ColorSettings::SetBlue(val) => ColorSettings::SetBlue(color.blue - val),
                ColorSettings::SetAlpha(val) => ColorSettings::SetAlpha(color.alpha - val),
            },

            "*" => match setting {
                ColorSettings::SetHue(val) => ColorSettings::SetHue(color.hue * val),
                ColorSettings::SetSaturation(val) => {
                    ColorSettings::SetSaturation(color.saturation * val)
                }
                ColorSettings::SetValue(val) => ColorSettings::SetValue(color.value * val),
                ColorSettings::SetRed(val) => ColorSettings::SetRed(color.red * val),
                ColorSettings::SetGreen(val) => ColorSettings::SetGreen(color.green * val),
                ColorSettings::SetBlue(val) => ColorSettings::SetBlue(color.blue * val),
                ColorSettings::SetAlpha(val) => ColorSettings::SetAlpha(color.alpha * val),
            },

            "/" => match setting {
                ColorSettings::SetHue(val) => ColorSettings::SetHue(color.hue / val),
                ColorSettings::SetSaturation(val) => {
                    ColorSettings::SetSaturation(color.saturation / val)
                }
                ColorSettings::SetValue(val) => ColorSettings::SetValue(color.value / val),
                ColorSettings::SetRed(val) => ColorSettings::SetRed(color.red / val),
                ColorSettings::SetGreen(val) => ColorSettings::SetGreen(color.green / val),
                ColorSettings::SetBlue(val) => ColorSettings::SetBlue(color.blue / val),
                ColorSettings::SetAlpha(val) => ColorSettings::SetAlpha(color.alpha / val),
            },

            _ => panic!("Invalid operator"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum ColorSettings {
    SetHue(i16),
    SetSaturation(i16),
    SetValue(i16),
    SetRed(i16),
    SetGreen(i16),
    SetBlue(i16),
    SetAlpha(i16),
}

impl ColorSettings {
    fn validate_change(&mut self) {
        match self {
            Self::SetHue(hue) => {
                if !(0..MAX_HUE).contains(hue) {
                    *hue = hue.rem_euclid(MAX_HUE);
                }
            }

            Self::SetSaturation(val) | Self::SetValue(val) | Self::SetAlpha(val) => {
                if *val < 0 {
                    *val = 0;
                } else if *val > MAX_SVA {
                    *val = MAX_SVA;
                }
            }

            Self::SetRed(val) | Self::SetGreen(val) | Self::SetBlue(val) => {
                if *val < 0 {
                    *val = 0;
                } else if *val > MAX_RGB {
                    *val = MAX_RGB;
                }
            }
        }
    }

    fn is_hsv(&self) -> bool {
        matches!(
            self,
            Self::SetHue(_) | Self::SetSaturation(_) | Self::SetValue(_)
        )
    }

    fn is_rgb(&self) -> bool {
        matches!(self, Self::SetRed(_) | Self::SetGreen(_) | Self::SetBlue(_))
    }
}
fn is_xx(s: &str) -> bool {
    s.len() == 2 && s.chars().nth(0) == s.chars().nth(1)
}

impl Color {
    fn is_valid_hex(hex: &str) -> bool {
        let hex = hex.to_uppercase();
        let mut chars = hex.chars();
        if chars.next() != Some('#') {
            return false;
        }
        for c in chars {
            if !c.is_digit(16) {
                return false;
            }
        }
        hex.len() == 4 || hex.len() == 5 || hex.len() == 7 || hex.len() == 9
    }

    fn to_full_hex(hex: &str) -> String {
        let hex = hex.to_uppercase();
        if hex.len() == 4 || hex.len() == 5 {
            let mut chars = hex.chars().skip(1);
            let mut new_hex = String::with_capacity(9);
            new_hex.push('#');
            for c in chars {
                new_hex.push(c);
                new_hex.push(c);
            }
            new_hex
        } else {
            hex
        }
    }

    fn from_hex(hex: &str) -> Result<Self, ColorError> {
        if !Self::is_valid_hex(hex) {
            return Err(ColorError::InvalidHex(hex.to_owned()));
        }

        let hex = Self::to_full_hex(hex);

        let red = i16::from_str_radix(&hex[1..3], 16).unwrap();
        let green = i16::from_str_radix(&hex[3..5], 16).unwrap();
        let blue = i16::from_str_radix(&hex[5..7], 16).unwrap();
        let alpha = if hex.len() == 9 {
            let a255 = i16::from_str_radix(&hex[7..9], 16).unwrap();
            let a100 = a255 as f32 / 255.0 * 100.0;
            a100.floor() as i16
        } else {
            100
        };

        let mut color = Self {
            alpha,
            red,
            green,
            blue,
            ..Default::default()
        };

        color.update_hsv();
        color.update_hex();

        Ok(color)
    }

    fn from_rgb(red: i16, green: i16, blue: i16) -> Self {
        todo!()
    }

    fn from_hsv(hue: f32, saturation: f32, value: f32) -> Self {
        todo!()
    }

    fn update_hsv(&mut self) {
        let rgb = Srgb::new(
            self.red as f32 / 255.0,
            self.green as f32 / 255.0,
            self.blue as f32 / 255.0,
        );

        let hsv: Hsv = rgb.into_color();

        self.hue = hsv.hue.into_positive_degrees() as i16;
        self.saturation = (hsv.saturation * 100.0) as i16;
        self.value = (hsv.value * 100.0) as i16;
    }

    fn update_rgb(&mut self) {
        let hsv = Hsv::new(
            self.hue as f32,
            self.saturation as f32 / 100.0,
            self.value as f32 / 100.0,
        );
        let rgb: Srgb = hsv.into_color();

        self.red = (rgb.red * 255.0) as i16;
        self.green = (rgb.green * 255.0) as i16;
        self.blue = (rgb.blue * 255.0) as i16;
    }

    fn update_hex(&mut self) {
        let r = format!("{:02X}", self.red);
        let g = format!("{:02X}", self.green);
        let b = format!("{:02X}", self.blue);
        let rgb_xx = is_xx(&r) && is_xx(&g) && is_xx(&b);

        if self.alpha == 100 {
            if rgb_xx {
                self.hex = format!(
                    "#{}{}{}",
                    r.chars().nth(0).unwrap(),
                    g.chars().nth(0).unwrap(),
                    b.chars().nth(0).unwrap()
                );
            } else {
                self.hex = format!("#{}{}{}", r, g, b);
            }
        } else {
            let a255 = self.alpha as f32 / 100.0 * 255.0;
            let alpha = a255.ceil() as i16;
            let a = format!("{:02X}", alpha);
            if rgb_xx && is_xx(&a) {
                self.hex = format!(
                    "#{}{}{}{}",
                    r.chars().nth(0).unwrap(),
                    g.chars().nth(0).unwrap(),
                    b.chars().nth(0).unwrap(),
                    a.chars().nth(0).unwrap()
                );
            } else {
                self.hex = format!("#{}{}{}{}", r, g, b, a);
            }
        }
    }

    fn update(&mut self, settings: Vec<ColorSettings>) -> &Self {
        for mut setting in settings {
            setting.validate_change();

            match setting {
                ColorSettings::SetHue(h) => self.hue = h,
                ColorSettings::SetSaturation(s) => self.saturation = s,
                ColorSettings::SetValue(v) => self.value = v,

                ColorSettings::SetRed(r) => self.red = r,
                ColorSettings::SetGreen(g) => self.green = g,
                ColorSettings::SetBlue(b) => self.blue = b,

                ColorSettings::SetAlpha(a) => self.alpha = a,
            }

            match setting {
                ColorSettings::SetHue(_)
                | ColorSettings::SetSaturation(_)
                | ColorSettings::SetValue(_)
                | ColorSettings::SetSaturation(_) => {
                    self.update_rgb();
                }

                ColorSettings::SetRed(_)
                | ColorSettings::SetGreen(_)
                | ColorSettings::SetBlue(_) => {
                    self.update_hsv();
                }

                _ => {}
            }

            self.update_hex();
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_color() {
        let hex = "#FF0000";
        let color = Color::from_hex(hex).unwrap();

        println!("{:?}", &color);

        assert_eq!(color.red, 255);
        assert_eq!(color.green, 0);
        assert_eq!(color.blue, 0);
        assert_eq!(color.alpha, 100);
        assert_eq!(color.hex, String::from("#F00"));

        let hex = "#FFFF0000";
        let color = Color::from_hex(hex).unwrap();

        println!("{:?}", &color);

        assert_eq!(color.red, 255);
        assert_eq!(color.green, 255);
        assert_eq!(color.blue, 0);
        assert_eq!(color.alpha, 0);
        assert_eq!(color.hex, String::from("#FF00"));

        let hex = "#F005";
        let mut color = Color::from_hex(hex).unwrap();

        assert_eq!(color.hex, String::from("#F005"));
    }

    #[test]
    fn updating_color() {
        let hex = "#F005";
        let mut color = Color::from_hex(hex).unwrap();

        println!("{:?}", &color);

        let changes = vec![ColorSettings::SetHue(120)];
        color.update(changes);

        println!("{:?}", &color);

        assert_eq!(color.hex, String::from("#0F05"))
    }

    #[test]
    fn updating_color_out_of_bounds() {
        let hex = "#F005";
        let mut color = Color::from_hex(hex).unwrap();

        println!("{:?}", &color);

        let changes = vec![
            ColorSettings::SetHue(-50),
            ColorSettings::SetSaturation(120),
        ];

        color.update(changes);

        println!("{:?}", &color);

        assert_eq!(color.hex, String::from("#FF00D455"))
    }

    #[test]
    fn changing_color() {
        let hex = "#F00";
        let mut color = Color::from_hex(hex).unwrap();

        let changes = vec![("/", ColorSettings::SetAlpha(2))];
        let applied_changes = changes
            .iter()
            // Change the operation and setting into a ColorChanges
            .map(|(op, setting)| ColorChanges::from_setting(op, setting))
            // Create new ColorSettings from the ColorChanges
            .map(|modification| modification.apply_change(&color))
            .collect::<Vec<ColorSettings>>();

        // println!("{:?}\n{:?}", changes, applied_changes);

        assert_eq!(applied_changes, vec![ColorSettings::SetAlpha(50)]);

        color.update(applied_changes);

        // println!("{:?}", &color);

        assert_eq!(color.hex, String::from("#FF000080"));

        let hex = "#F00";

        let mut color = Color::from_hex("#F00").unwrap();

        let changes = vec![("/", ColorSettings::SetAlpha(3))];

        let applied_changes = changes
            .iter()
            .map(|(op, setting)| ColorChanges::apply_to_setting(op, &color, setting))
            .collect::<Vec<ColorSettings>>();

        assert_eq!(applied_changes, vec![ColorSettings::SetAlpha(33)]);

        color.update(applied_changes);

        assert_eq!(color.hex, String::from("#F005"));
    }
}
