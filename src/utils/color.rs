/**
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
use color_name::Color as ColorName;
use palette::{Hsl, Hsv, IntoColor, Srgb};
use std::fmt;
use std::hash::Hash;
use std::ops::{Add, BitAnd, Div, Mul, Sub};
use std::str::FromStr;

const MAX_RGB: i16 = 255;
const MAX_SVA: i16 = 100;
const MAX_HUE: i16 = 360;

/// Checks if a string is XX where X is any character
fn is_xx(s: &str) -> bool {
    s.len() == 2 && s.chars().next() == s.chars().nth(1)
}

pub type Operations = Vec<Operation>;

#[derive(Debug)]
pub enum Error {
    Hex(String),
    Component,
    Change,
    Operator,
    InvalidColorString,
}

#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub enum Component {
    Hue(i16),
    Saturation(i16),
    Value(i16),
    Red(i16),
    Green(i16),
    Blue(i16),
    Lightness(i16),
    Alpha(i16),
    Hex(String),
}

macro_rules! impl_color_components_op {
    ($trait: ident, $func_name: ident, $op: tt) => {
        impl $trait<Component> for &Color {
            type Output = Component;

            fn $func_name(self, rhs: Component) -> Self::Output {
                match rhs {
                    Component::Hue(val) => Component::Hue(self.hue $op val),
                    Component::Saturation(val) => {
                        Component::Saturation(self.saturation $op val)
                    }
                    Component::Value(val) => Component::Value(self.value $op val),
                    Component::Red(val) => Component::Red(self.red $op val),
                    Component::Green(val) => Component::Green(self.green $op val),
                    Component::Blue(val) => Component::Blue(self.blue $op val),
                    Component::Alpha(val) => Component::Alpha(self.alpha $op val),
                    Component::Lightness(val) => Component::Lightness(self.lightness $op val),
                    Component::Hex(_) => unreachable!(),
                }
            }
        }
    };
}

impl_color_components_op!(Add, add, +);
impl_color_components_op!(Div, div, /);
impl_color_components_op!(Mul, mul, *);
impl_color_components_op!(Sub, sub, -);

impl BitAnd<Component> for &Color {
    type Output = Component;

    fn bitand(self, rhs: Component) -> Self::Output {
        if let Component::Hex(val) = rhs {
            Component::Hex(format!("{}{}", self.to_alphaless_hex(), val))
        } else {
            unreachable!()
        }
    }
}

impl Component {
    fn validate_change(&mut self) {
        match self {
            Self::Hue(hue) => {
                if !(0..MAX_HUE).contains(hue) {
                    *hue = hue.rem_euclid(MAX_HUE);
                }
            }

            Self::Saturation(val) | Self::Value(val) | Self::Alpha(val) | Self::Lightness(val) => {
                *val = (*val).clamp(0, MAX_SVA);
            }

            Self::Red(val) | Self::Green(val) | Self::Blue(val) => {
                *val = (*val).clamp(0, MAX_RGB);
            }

            Self::Hex(_) => {}
        }
    }
}

/**
Color Changes are represented mainly by
(Component Operator Value)

Examples
    hue+10 saturation/2 red=2

Components can be represented by the first letter of their name

Examples
    h+10 s/50 r=2
*/
#[derive(Debug, PartialEq, Clone, Hash, Eq)]
pub struct Operation(pub Component, pub String);

// let applied_changes = vec![Operation(Component::Alpha(3), "/")];
// let applied_changes = vec![color_change!(Alpha "/", 3)];
// let applied_changes = color_change!(Alpha "/", 3;)
#[macro_export]
macro_rules! operation {
    ($setting: ident . $val: expr) => {
        Operation(Component::$setting($val), String::from("."))
    };
    ($setting: ident=$val: expr) => {
        Operation(Component::$setting($val), String::from("="))
    };
    ($setting: ident $op: expr; $val: expr) => {
        Operation(Component::$setting($val), $op.to_string())
    };
    ($setting: ident: $op: expr, $val: expr;) => {
        vec![color_change!($setting $op; $val)]
    };
    ($($setting: ident: $op: expr, $val: expr);*) => {
        vec![$(color_change!($setting $op; $val)),*]
    };
    ($setting: ident $val: expr,) => {
        vec![color_change!($setting=$val)]
    };
    ($($setting: ident $val: expr),+) => {
        vec![$(color_change!($setting=$val)),+]
    };
}

impl Operation {
    fn apply(self, color: &Color) -> Result<Component, Error> {
        let new_change = match self.1.as_str() {
            "+" => color + self.0,
            "-" => color - self.0,
            "=" => self.0,
            "/" => color / self.0,
            "*" => color * self.0,
            "." => color & self.0,
            _ => return Err(Error::Change),
        };

        Ok(new_change)
    }

    pub fn identity(c: Self) -> Self {
        match (&c.0, c.1.as_str()) {
            (Component::Alpha(_), "=") | (Component::Hex(_), ".") => operation!(Alpha = 100),
            _ => c,
        }
    }

    pub fn inverse(changes: &Operations) -> Operations {
        changes
            .iter()
            .cloned()
            .map(|c| match (&c.0, c.1.as_str()) {
                (_, "+") => Self(c.0, String::from("-")),
                (_, "-") => Self(c.0, String::from("+")),
                (_, "/") => Self(c.0, String::from("*")),
                (_, "*") => Self(c.0, String::from("/")),
                (Component::Alpha(_), "=") | (Component::Hex(_), ".") => operation!(Alpha = 100),
                _ => c,
            })
            .rev()
            .collect()
    }

    pub fn inverse_ops(changes: &[&Operations]) -> Vec<Operations> {
        changes.iter().map(|c| Self::inverse(c)).collect()
    }

    pub fn identity_op(changes: &Operations) -> Operations {
        changes.iter().map(|c| Self::identity(c.clone())).collect()
    }

    pub fn identity_ops(changes: &[&Operations]) -> Vec<Operations> {
        changes
            .iter()
            .map(|c| c.iter().map(|c| Self::identity(c.clone())).collect())
            .collect()
    }
}

const fn get_operator(op: Option<char>) -> Result<&'static str, Error> {
    match op {
        Some(c) => match c {
            '+' => Ok("+"),
            '-' => Ok("-"),
            '=' => Ok("="),
            '*' => Ok("*"),
            '/' => Ok("/"),
            '.' => Ok("."),
            _ => Err(Error::Operator),
        },
        None => Err(Error::Operator),
    }
}

impl FromStr for Operation {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let mut chars = s.chars();
        let component_char = chars.next();

        let mut chars = chars.skip_while(|c| c.is_alphabetic());
        let op = get_operator(chars.next())?;

        // Special Alpha Append Operator
        // #FF0000..XX == #FF0000XX
        // VS Normal Alpha Set
        // #FF0000.=XXX == #FF0000..(XXX * 2.55) (Alpha is 0-100)
        if op == "." {
            let val = chars.collect::<String>();
            return Ok(operation!(Hex.val));
        }

        let val: i16 = chars
            .collect::<String>()
            .parse()
            .map_err(|_| Error::Change)?;

        let component = match component_char {
            Some('h') => Component::Hue(val),
            Some('s') => Component::Saturation(val),
            Some('v') => Component::Value(val),
            Some('l') => Component::Lightness(val),
            Some('r') => Component::Red(val),
            Some('g') => Component::Green(val),
            Some('b') => Component::Blue(val),
            Some('a') => Component::Alpha(val),
            _ => return Err(Error::Component),
        };

        Ok(Self(component, op.to_string()))
    }
}

#[derive(Debug, Clone, Eq)]
pub struct Color {
    alpha: i16,
    red: i16,
    green: i16,
    blue: i16,
    hue: i16,
    saturation: i16,
    lightness: i16,
    value: i16,
    pub hex: String,
}

const fn in_range(a: i16, b: i16, r: i16) -> bool {
    (a - b).abs() <= r
}

impl PartialEq for Color {
    fn eq(&self, other: &Self) -> bool {
        let max_distance = 1;
        ((in_range(self.hue, other.hue, max_distance)
            && in_range(self.saturation, other.saturation, max_distance)
            && in_range(self.value, other.value, max_distance))
            || (in_range(self.red, other.red, max_distance)
                && in_range(self.green, other.green, max_distance)
                && in_range(self.blue, other.blue, max_distance)))
            && self.alpha == other.alpha
    }
}

impl Hash for Color {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.alpha.hash(state);
        self.red.hash(state);
        self.green.hash(state);
        self.blue.hash(state);
        self.hue.hash(state);
        self.lightness.hash(state);
        self.saturation.hash(state);
        self.value.hash(state);
    }
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
            lightness: 0,
            value: 0,
            hex: String::from("#000"),
        }
    }
}

impl FromStr for Color {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(ColorType::from_str(s)?)
    }
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
impl Color {
    pub fn from_change(col_str: &str, ops: &[Operation]) -> Result<Self, Error> {
        let mut color = col_str.parse::<Self>()?;
        color.update(ops.to_vec())?;
        Ok(color)
    }

    fn is_valid_hex(hex: &str) -> bool {
        let hex = hex.to_uppercase();
        hex.starts_with('#')
            && hex.chars().skip(1).all(|c| c.is_ascii_hexdigit())
            && matches!(hex.len(), 4 | 5 | 7 | 9)
    }

    pub fn norm_hex(hex: &str) -> String {
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

    pub fn get_alpha(&self) -> String {
        let alpha = f32::from(self.alpha) * 2.55;
        format!("{:02X}", alpha.ceil() as u8)
    }

    pub const fn has_alpha(&self) -> bool {
        self.alpha != 100
    }

    pub fn to_alphaless_hex(&self) -> String {
        if self.has_alpha() {
            format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
        } else {
            Self::norm_hex(&self.hex)
        }
    }

    pub fn get_name(&self) -> String {
        let rgb: [u8; 3] = [self.red as u8, self.green as u8, self.blue as u8];
        format!("color.{}", ColorName::similar(rgb).to_lowercase())
    }

    pub fn from_hex(hex: &str) -> Result<Self, Error> {
        if !Self::is_valid_hex(hex) {
            return Err(Error::Hex(hex.to_owned()));
        }

        let hex = Self::norm_hex(hex);

        let red = i16::from_str_radix(&hex[1..3], 16).map_err(|_| Error::Hex(hex.clone()))?;
        let green = i16::from_str_radix(&hex[3..5], 16).map_err(|_| Error::Hex(hex.clone()))?;
        let blue = i16::from_str_radix(&hex[5..7], 16).map_err(|_| Error::Hex(hex.clone()))?;

        let alpha = if hex.len() == 9 {
            let a255 = i16::from_str_radix(&hex[7..9], 16).map_err(|_| Error::Hex(hex.clone()))?;
            let a100 = f32::from(a255) / 255.0 * 100.0;
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

        color.update_hsvl();
        color.update_hex();

        Ok(color)
    }

    fn update_value(&mut self) {
        let hs_light = Hsl::new(
            f32::from(self.hue),
            f32::from(self.saturation) / 100.0,
            f32::from(self.lightness) / 100.0,
        );
        let hs_val: Hsv = hs_light.into_color();

        self.saturation = (hs_val.saturation * 100.0) as i16;
        self.value = (hs_val.value * 100.0) as i16;
    }

    fn update_lightness(&mut self) {
        let hs_val = Hsv::new(
            f32::from(self.hue),
            f32::from(self.saturation) / 100.0,
            f32::from(self.value) / 100.0,
        );
        let hs_light: Hsl = hs_val.into_color();

        self.lightness = (hs_light.lightness * 100.0) as i16;
    }

    fn update_hsvl(&mut self) {
        let rgb = Srgb::new(
            f32::from(self.red) / 255.0,
            f32::from(self.green) / 255.0,
            f32::from(self.blue) / 255.0,
        );

        let hs_val: Hsv = rgb.into_color();
        let hs_light: Hsl = rgb.into_color();

        self.hue = hs_val.hue.into_positive_degrees() as i16;
        self.saturation = (hs_val.saturation * 100.0) as i16;
        self.value = (hs_val.value * 100.0) as i16;
        self.lightness = (hs_light.lightness * 100.0) as i16;
    }

    fn update_rgb(&mut self) {
        let hsv = Hsv::new(
            f32::from(self.hue),
            f32::from(self.saturation) / 100.0,
            f32::from(self.value) / 100.0,
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
                self.hex = format!("#{r}{g}{b}");
            }
        } else {
            let a255 = f32::from(self.alpha) / 100.0 * 255.0;
            let alpha = a255.ceil() as i16;
            let a = format!("{alpha:02X}");
            if rgb_xx && is_xx(&a) {
                self.hex = format!(
                    "#{}{}{}{}",
                    r.chars().next().unwrap(),
                    g.chars().next().unwrap(),
                    b.chars().next().unwrap(),
                    a.chars().next().unwrap()
                );
            } else {
                self.hex = format!("#{r}{g}{b}{a}");
            }
        }
    }

    pub fn update_ops(&mut self, changes: &[Operations]) -> Result<(), Error> {
        changes
            .iter()
            .try_for_each(|change| self.update(change.clone()))?;

        Ok(())
    }

    pub fn update(&mut self, changes: Operations) -> Result<(), Error> {
        for change in changes {
            let mut setting = change.apply(self)?;

            setting.validate_change();

            match setting {
                Component::Hue(h) => self.hue = h,
                Component::Saturation(s) => self.saturation = s,
                Component::Value(v) => self.value = v,
                Component::Lightness(l) => self.lightness = l,

                Component::Red(r) => self.red = r,
                Component::Green(g) => self.green = g,
                Component::Blue(b) => self.blue = b,

                Component::Alpha(a) => self.alpha = a,
                Component::Hex(_) => {}
            }

            match setting {
                Component::Hue(_) | Component::Saturation(_) | Component::Value(_) => {
                    self.update_lightness();
                    self.update_rgb();
                    self.update_hex();
                }

                Component::Lightness(_) => {
                    self.update_value();
                    self.update_rgb();
                    self.update_hex();
                }

                Component::Red(_) | Component::Green(_) | Component::Blue(_) => {
                    self.update_hsvl();
                    self.update_hex();
                }

                Component::Alpha(_) => self.update_hex(),
                Component::Hex(ref hex) => self.clone_from(&Self::from_hex(hex)?),
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
enum ColorType {
    Hex(String),
    Hsl(i16, i16, i16, i16),
    Hsv(i16, i16, i16, i16),
    Rgb(i16, i16, i16, i16),
}

impl FromStr for ColorType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let s = s.to_lowercase();

        if s.starts_with('#') {
            return Ok(Self::Hex(s));
        }

        if !s.ends_with(')') {
            return Err(Error::InvalidColorString);
        }

        let splits: Vec<_> = s.split_terminator(&['(', ',', ')']).collect();
        if splits.len() < 2 {
            return Err(Error::InvalidColorString);
        }

        let color_values = &splits[1..];
        if color_values.len() < 3 {
            return Err(Error::InvalidColorString);
        }

        let mut color_type = splits[0].to_string();
        color_type.truncate(3);

        let color_values = color_values
            .iter()
            .map(|c| c.trim().parse::<i16>().unwrap())
            .collect::<Vec<_>>();

        let alpha = color_values.get(3).unwrap_or(&100);

        match color_type.as_str() {
            "rgb" => Ok(Self::Rgb(
                color_values[0],
                color_values[1],
                color_values[2],
                *alpha,
            )),
            "hsl" => Ok(Self::Hsl(
                color_values[0],
                color_values[1],
                color_values[2],
                *alpha,
            )),
            "hsv" => Ok(Self::Hsv(
                color_values[0],
                color_values[1],
                color_values[2],
                *alpha,
            )),
            _ => Err(Error::InvalidColorString),
        }
    }
}

impl TryFrom<ColorType> for Color {
    type Error = Error;
    fn try_from(value: ColorType) -> Result<Self, Self::Error> {
        match value {
            ColorType::Hex(hex) => Self::from_hex(&hex),
            ColorType::Hsl(h, s, l, a) => {
                let mut color = Self {
                    hue: h,
                    saturation: s,
                    lightness: l,
                    alpha: a,
                    ..Default::default()
                };
                color.update_value();
                color.update_rgb();
                color.update_hex();
                Ok(color)
            }
            ColorType::Hsv(h, s, v, a) => {
                let mut color = Self {
                    hue: h,
                    saturation: s,
                    value: v,
                    alpha: a,
                    ..Default::default()
                };
                color.update_lightness();
                color.update_rgb();
                color.update_hex();
                Ok(color)
            }
            ColorType::Rgb(r, g, b, a) => {
                let mut color = Self {
                    red: r,
                    green: g,
                    blue: b,
                    alpha: a,
                    ..Default::default()
                };
                color.update_hsvl();
                color.update_hex();
                Ok(color)
            }
        }
    }
}
