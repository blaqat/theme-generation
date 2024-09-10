use palette::{Hsv, IntoColor, Srgb};
use std::fmt;
use std::ops::{Add, Div, Mul, Sub};
use std::path::Display;
use std::str::FromStr;

/*
Colors are hex strings that have easy access to their hue, saturation, value, and lightness.
Valid Color Strings Include:
- #RRGGBB
- #RGB
- #RRGGBBAA
- #RGBA

When color is modified, all properties are updated.
e.g if hsv is modified rgb are updated
if rgb is modified hsvl are updated
*/

const MAX_RGB: i16 = 255;
const MAX_SVA: i16 = 100;
const MAX_HUE: i16 = 360;

fn is_xx(s: &str) -> bool {
    s.len() == 2 && s.chars().next() == s.chars().nth(1)
}

pub type ColorOperations<'a> = Vec<ColorChange<'a>>;

#[derive(Debug)]
pub enum ColorError {
    Hex(String),
    ColorComponent,
    ColorChange,
    ColorOperator,
}

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum ColorComponent {
    Hue(i16),
    Saturation(i16),
    Value(i16),
    Red(i16),
    Green(i16),
    Blue(i16),
    Alpha(i16),
}

macro_rules! impl_color_components_op {
    ($trait: ident, $func_name: ident, $op: tt) => {
        impl $trait<ColorComponent> for &Color {
            type Output = ColorComponent;

            fn $func_name(self, rhs: ColorComponent) -> Self::Output {
                match rhs {
                    ColorComponent::Hue(val) => ColorComponent::Hue(self.hue $op val),
                    ColorComponent::Saturation(val) => {
                        ColorComponent::Saturation(self.saturation $op val)
                    }
                    ColorComponent::Value(val) => ColorComponent::Value(self.value $op val),
                    ColorComponent::Red(val) => ColorComponent::Red(self.red $op val),
                    ColorComponent::Green(val) => ColorComponent::Green(self.green $op val),
                    ColorComponent::Blue(val) => ColorComponent::Blue(self.blue $op val),
                    ColorComponent::Alpha(val) => ColorComponent::Alpha(self.alpha $op val),
                }
            }
        }
    };
}

impl_color_components_op!(Add, add, +);
impl_color_components_op!(Div, div, /);
impl_color_components_op!(Mul, mul, *);
impl_color_components_op!(Sub, sub, -);

impl ColorComponent {
    fn validate_change(&mut self) {
        match self {
            Self::Hue(hue) => {
                if !(0..MAX_HUE).contains(hue) {
                    *hue = hue.rem_euclid(MAX_HUE);
                }
            }

            Self::Saturation(val) | Self::Value(val) | Self::Alpha(val) => {
                *val = (*val).clamp(0, MAX_SVA);
            }

            Self::Red(val) | Self::Green(val) | Self::Blue(val) => {
                *val = (*val).clamp(0, MAX_RGB);
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct ColorChange<'a>(ColorComponent, &'a str);

// let applied_changes = vec![ColorChange(ColorComponent::Alpha(3), "/")];
// let applied_changes = vec![color_change!(Alpha "/", 3)];
macro_rules! color_change {
    ($setting: ident=$val: expr) => {
        ColorChange(ColorComponent::$setting($val), "=")
    };
    ($setting: ident $op: expr; $val: expr) => {
        ColorChange(ColorComponent::$setting($val), $op)
    };
    ($($setting: ident: $op: expr, $val: expr);+) => {
        vec![$(color_change!($setting $op; $val)),+]
    };
    ($($setting: ident $val: expr),+) => {
        vec![$(color_change!($setting=$val)),+]
    };
}

impl<'a> ColorChange<'a> {
    fn apply(self, color: &'a Color) -> Result<ColorComponent, ColorError> {
        let new_change = match self.1 {
            "+" => color + self.0,
            "-" => color - self.0,
            "=" => self.0,
            "/" => color / self.0,
            _ => return Err(ColorError::ColorChange),
        };

        Ok(new_change)
    }

    pub fn identity(c: ColorChange) -> ColorChange {
        match (&c.0, c.1) {
            (ColorComponent::Alpha(_), "=") => color_change!(Alpha = 100),
            _ => c,
        }
    }

    pub fn inverse(changes: &'a ColorOperations) -> ColorOperations<'a> {
        changes
            .iter()
            .cloned()
            .map(|c| match (&c.0, c.1) {
                (_, "+") => ColorChange(c.0, "-"),
                (_, "-") => ColorChange(c.0, "+"),
                (_, "/") => ColorChange(c.0, "*"),
                (_, "*") => ColorChange(c.0, "/"),
                (ColorComponent::Alpha(_), "=") => color_change!(Alpha = 100),
                _ => c,
            })
            .rev()
            .collect()
    }

    pub fn inverse_ops(changes: Vec<&'a ColorOperations>) -> Vec<ColorOperations<'a>> {
        changes.iter().map(|c| ColorChange::inverse(c)).collect()
    }

    pub fn identitiy_ops(changes: Vec<&'a ColorOperations>) -> Vec<ColorOperations<'a>> {
        changes
            .iter()
            .map(|c| c.iter().map(|c| ColorChange::identity(c.clone())).collect())
            .collect()
    }
}

fn get_operator(op: Option<char>) -> Result<&'static str, ColorError> {
    match op {
        Some(c) => match c {
            '+' => Ok("+"),
            '-' => Ok("-"),
            '=' => Ok("="),
            '*' => Ok("*"),
            '/' => Ok("/"),
            '.' => Ok("."),
            _ => Err(ColorError::ColorOperator),
        },
        None => Err(ColorError::ColorOperator),
    }
}

/*
Color Changes are represented mainly by
Component Operator Value
Examples
hue+10 saturation/2 red=2

Components can be represented by the first letter of their name
Examples
h+10 s/50 r=2
*/
impl<'a> FromStr for ColorChange<'a> {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut chars = s.chars();
        let component_char = chars.next();

        let mut chars = chars.skip_while(|c| c.is_alphabetic());
        let op = get_operator(chars.next())?;

        // Special Alpha Append Operator
        if op == "." {
            let val: i16 = i16::from_str_radix(&chars.collect::<String>(), 16)
                .map_err(|_| ColorError::ColorOperator)?;
            return Ok(color_change!(Alpha = val));
        }

        let val: i16 = chars.collect::<String>().parse().unwrap();

        let component = match component_char {
            Some('h') => ColorComponent::Hue(val),
            Some('s') => ColorComponent::Saturation(val),
            Some('v') => ColorComponent::Value(val),
            Some('r') => ColorComponent::Red(val),
            Some('g') => ColorComponent::Green(val),
            Some('b') => ColorComponent::Blue(val),
            Some('a') => ColorComponent::Alpha(val),
            _ => return Err(ColorError::ColorComponent),
        };

        Ok(Self(component, op))
    }
}

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct Color {
    alpha: i16,
    red: i16,
    green: i16,
    blue: i16,
    hue: i16,
    saturation: i16,
    value: i16,
    pub hex: String,
}
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.hex)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self {
            alpha: 100,
            red: 0,
            green: 0,
            blue: 0,
            hue: 0,
            saturation: 0,
            value: 0,
            hex: "#000000".to_string(),
        }
    }
}

impl FromStr for Color {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_hex(s)
    }
}

impl Color {
    fn is_valid_hex(hex: &str) -> bool {
        let hex = hex.to_uppercase();
        hex.starts_with("#")
            && hex.chars().skip(1).all(|c| c.is_ascii_hexdigit())
            && matches!(hex.len(), 4 | 5 | 7 | 9)
    }

    fn to_full_hex(hex: &str) -> String {
        let hex = hex.to_uppercase();
        if hex.len() == 4 || hex.len() == 5 {
            let mut new_hex = String::with_capacity(9);
            new_hex.push('#');
            for c in hex.chars().skip(1) {
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
            return Err(ColorError::Hex(hex.to_owned()));
        }

        let hex = Self::to_full_hex(hex);

        let red =
            i16::from_str_radix(&hex[1..3], 16).map_err(|_| ColorError::Hex(hex.to_owned()))?;
        let green =
            i16::from_str_radix(&hex[3..5], 16).map_err(|_| ColorError::Hex(hex.to_owned()))?;
        let blue =
            i16::from_str_radix(&hex[5..7], 16).map_err(|_| ColorError::Hex(hex.to_owned()))?;

        let alpha = if hex.len() == 9 {
            let a255 =
                i16::from_str_radix(&hex[7..9], 16).map_err(|_| ColorError::Hex(hex.to_owned()))?;
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
        let rgb_xx = [&r, &g, &b].iter().all(|c| is_xx(c));

        if self.alpha == 100 {
            if rgb_xx {
                self.hex = format!(
                    "#{}{}{}",
                    r.chars().next().unwrap(),
                    g.chars().next().unwrap(),
                    b.chars().next().unwrap()
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
                    r.chars().next().unwrap(),
                    g.chars().next().unwrap(),
                    b.chars().next().unwrap(),
                    a.chars().next().unwrap()
                );
            } else {
                self.hex = format!("#{}{}{}{}", r, g, b, a);
            }
        }
    }

    pub fn update_ops(&mut self, changes: &[ColorOperations]) -> Result<(), ColorError> {
        changes
            .iter()
            .try_for_each(|change| self.update(change.clone()))?;

        Ok(())
    }

    pub fn update(&mut self, changes: ColorOperations) -> Result<(), ColorError> {
        for change in changes {
            let mut setting = change.apply(self)?;

            setting.validate_change();

            match setting {
                ColorComponent::Hue(h) => self.hue = h,
                ColorComponent::Saturation(s) => self.saturation = s,
                ColorComponent::Value(v) => self.value = v,

                ColorComponent::Red(r) => self.red = r,
                ColorComponent::Green(g) => self.green = g,
                ColorComponent::Blue(b) => self.blue = b,

                ColorComponent::Alpha(a) => self.alpha = a,
            }

            match setting {
                ColorComponent::Hue(_)
                | ColorComponent::Saturation(_)
                | ColorComponent::Value(_)
                | ColorComponent::Saturation(_) => {
                    self.update_rgb();
                }

                ColorComponent::Red(_) | ColorComponent::Green(_) | ColorComponent::Blue(_) => {
                    self.update_hsv();
                }

                _ => {}
            }

            self.update_hex();
        }

        Ok(())
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

        // let changes = vec![color_change!(Hue 120)];
        let changes = color_change![ Hue 120 ];

        color.update(changes);

        println!("{:?}", &color);

        assert_eq!(color.hex, String::from("#0F05"))
    }

    #[test]
    fn updating_color_out_of_bounds() {
        let hex = "#F005";
        let mut color = Color::from_hex(hex).unwrap();

        println!("{:?}", &color);

        // let changes = vec![ColorChange(ColorComponent::Hue(-50), "="), ColorChange(ColorComponent::Saturation(120), "=")];
        // let changes = vec![color_change!(Hue=-50), color_change!(Saturation=120)];
        let changes = color_change![ Hue -50, Saturation 120 ];

        color.update(changes);

        println!("{:?}", &color);

        assert_eq!(color.hex, String::from("#FF00D455"))
    }

    #[test]
    fn changing_color() {
        let hex = "#F00";
        let mut color = Color::from_hex(hex).unwrap();

        let applied_changes = color_change! {
            Alpha: "/", 3;
            Saturation: "-", 10
        };

        color.update(applied_changes);

        assert_eq!(color.hex, String::from("#FF191955"));
    }
}
