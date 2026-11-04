// SPDX-License-Identifier: GPL-2.0-or-later
// MSI 761-byte protocol adapted from OpenRGB's MSIMotherboard761Controller
// (rom4ster, 2025), GPL-2.0-or-later. See THIRD_PARTY.md for exact sources.
// Restricted to the positively identified MS-7E51 controller on this PC.
use hidapi::{HidApi, HidDevice};
use std::io;

pub const DEVICE_ID: u32 = 0x8000_7e51;
pub const LEDS_PER_ZONE: usize = 240;
pub const LEDS: usize = 4 * LEDS_PER_ZONE;

pub struct MsiBoard {
    device: HidDevice,
    config: [u8; 290],
    initialized: bool,
}

fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

impl MsiBoard {
    /// Enumeration and feature reads only. No light changes until `enable`.
    pub fn discover() -> io::Result<Option<Self>> {
        let api = HidApi::new().map_err(err)?;
        let mut matching = api.device_list().filter(|d| {
            d.vendor_id() == 0x0db0
                && d.product_id() == 0x0076
                && d.usage_page() == 0xff00
                && d.usage() == 1
                && d.serial_number().is_some_and(|s| s.starts_with("7E51"))
        });
        let Some(info) = matching.next() else {
            return Ok(None);
        };
        if matching.next().is_some() {
            return Err(err("偵測到多個 MS-7E51 控制器，請先確認裝置"));
        }
        let device = info.open_device(&api).map_err(err)?;
        let mut config = [0u8; 290];
        config[0] = 0x50;
        let count = device.get_feature_report(&mut config).map_err(err)?;
        if count != config.len() || config[0] != 0x50 {
            return Err(err(format!("MS-7E51 設定回覆格式不符（{count} bytes）")));
        }
        Ok(Some(Self {
            device,
            config,
            initialized: false,
        }))
    }

    pub fn enable(&mut self) -> io::Result<()> {
        if self.initialized {
            return Ok(());
        }
        // Preserve all unrelated headers. Enable direct mode on JARGB1/2/3 and JAF
        // with BLACK initial colors, so initialization never flashes full white.
        let mut config = self.config;
        for zone in 0..4 {
            let offset = 1 + zone * 16;
            config[offset..offset + 16].copy_from_slice(&[
                0x09, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x03, 0x15, 0x78,
            ]);
        }
        self.device.send_feature_report(&config).map_err(err)?;
        self.initialized = true;
        Ok(())
    }

    pub fn colors(&mut self, colors: &[[u8; 3]]) -> io::Result<()> {
        if colors.len() != LEDS {
            return Err(err("MS-7E51 色彩緩衝區大小不符"));
        }
        self.enable()?;
        for zone in 0..4 {
            self.device
                .send_feature_report(&color_packet(
                    zone,
                    &colors[zone * LEDS_PER_ZONE..(zone + 1) * LEDS_PER_ZONE],
                )?)
                .map_err(err)?;
        }
        Ok(())
    }

    pub fn poll(&self) -> io::Result<()> {
        self.device.get_product_string().map_err(err)?;
        Ok(())
    }
}

fn color_packet(zone: usize, colors: &[[u8; 3]]) -> io::Result<[u8; 727]> {
    if zone >= 4 || colors.len() != LEDS_PER_ZONE {
        return Err(err("ARGB 封包範圍不符"));
    }
    let mut packet = [0; 727];
    // UI ordering: JAF, JARGB1, JARGB2, JARGB3.
    packet[..7].copy_from_slice(&[
        0x51,
        0x09,
        if zone == 0 { 0x08 } else { 0x04 },
        zone.saturating_sub(1) as u8,
        0,
        0,
        240,
    ]);
    for (index, rgb) in colors.iter().enumerate() {
        packet[7 + index * 3..10 + index * 3].copy_from_slice(rgb);
    }
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zone_headers_and_last_led_match_documented_wire_format() {
        let mut colors = [[0, 0, 0]; LEDS_PER_ZONE];
        colors[239] = [7, 8, 9];
        for (zone, kind, index) in [(0, 8, 0), (1, 4, 0), (2, 4, 1), (3, 4, 2)] {
            let p = color_packet(zone, &colors).unwrap();
            assert_eq!(&p[..7], &[0x51, 9, kind, index, 0, 0, 240]);
            assert_eq!(&p[724..727], &[7, 8, 9]);
        }
        assert!(color_packet(4, &colors).is_err());
        assert!(color_packet(0, &colors[..239]).is_err());
    }
}
