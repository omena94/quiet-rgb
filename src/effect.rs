use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    #[default]
    Solid,
    Breathe,
    Rainbow,
    Wave,
    Off,
}

impl Effect {
    pub const ALL: [Self; 5] = [
        Self::Solid,
        Self::Breathe,
        Self::Rainbow,
        Self::Wave,
        Self::Off,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Solid => "固定色",
            Self::Breathe => "呼吸",
            Self::Rainbow => "彩虹",
            Self::Wave => "流動彩虹",
            Self::Off => "熄燈",
        }
    }
    pub fn animated(self) -> bool {
        matches!(self, Self::Breathe | Self::Rainbow | Self::Wave)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub effect: Effect,
    pub color: [u8; 3],
    pub brightness: f32,
    pub period: f32,
    pub off_on_exit: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            effect: Effect::Solid,
            color: [110, 175, 255],
            brightness: 0.15,
            period: 4.0,
            off_on_exit: true,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.brightness = if self.brightness.is_finite() {
            self.brightness.clamp(0.0, 1.0)
        } else {
            0.15
        };
        self.period = if self.period.is_finite() {
            self.period.clamp(0.5, 15.0)
        } else {
            4.0
        };
    }
    pub fn colors(&self, count: usize, seconds: f32) -> Vec<[u8; 3]> {
        let phase = seconds / self.period.max(0.5);
        let level = self.brightness.clamp(0.0, 1.0)
            * match self.effect {
                Effect::Off => 0.0,
                Effect::Breathe => (1.0 - (phase * std::f32::consts::TAU).cos()) * 0.5,
                _ => 1.0,
            };
        (0..count)
            .map(|i| {
                let rgb = match self.effect {
                    Effect::Rainbow => hue(phase),
                    Effect::Wave => hue(phase + i as f32 / count.max(1) as f32),
                    _ => self.color,
                };
                rgb.map(|c| (c as f32 * level).round() as u8)
            })
            .collect()
    }
}

fn hue(value: f32) -> [u8; 3] {
    let h = value.rem_euclid(1.0) * 6.0;
    let x = ((1.0 - (h % 2.0 - 1.0).abs()) * 255.0).round() as u8;
    match h as u8 {
        0 => [255, x, 0],
        1 => [x, 255, 0],
        2 => [0, 255, x],
        3 => [0, x, 255],
        4 => [x, 0, 255],
        _ => [255, 0, x],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brightness_caps_every_effect_and_off_is_black() {
        for effect in Effect::ALL {
            let s = Settings {
                effect,
                brightness: 0.1,
                ..Default::default()
            };
            for t in 0..100 {
                let colors = s.colors(20, t as f32 * 0.1);
                assert!(colors.iter().flatten().all(|c| *c <= 26));
                if effect == Effect::Off {
                    assert!(colors.iter().flatten().all(|c| *c == 0));
                }
            }
        }
    }
    #[test]
    fn breathing_has_dark_and_bright_extrema_and_wraps() {
        let s = Settings {
            effect: Effect::Breathe,
            color: [200, 100, 50],
            brightness: 0.5,
            period: 4.0,
            ..Default::default()
        };
        assert_eq!(s.colors(1, 0.0), [[0, 0, 0]]);
        assert_eq!(s.colors(1, 2.0), [[100, 50, 25]]);
        assert_eq!(s.colors(1, 4.0), [[0, 0, 0]]);
    }
    #[test]
    fn rainbow_is_uniform_but_wave_is_spatial() {
        let mut s = Settings {
            effect: Effect::Rainbow,
            ..Default::default()
        };
        let colors = s.colors(6, 0.0);
        assert!(colors.windows(2).all(|c| c[0] == c[1]));
        s.effect = Effect::Wave;
        assert_ne!(s.colors(6, 0.0)[0], s.colors(6, 0.0)[3]);
    }
}
