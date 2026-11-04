use crate::{
    msi::{self, MsiBoard},
    openrgb::{Client, Device},
};
use std::io;

pub struct Backend {
    native: Option<MsiBoard>,
    sdk: Option<Client>,
    sdk_count: usize,
}

impl Backend {
    pub fn connect(port: u16) -> io::Result<(Self, Vec<Device>)> {
        let native_result = MsiBoard::discover();
        let sdk_result = Client::connect(port);
        let mut devices = Vec::new();
        let mut messages = Vec::new();
        let native = match native_result {
            Ok(Some(board)) => {
                devices.push(Device::msi_board());
                Some(board)
            }
            Ok(None) => None,
            Err(e) => {
                messages.push(format!("主機板 USB：{e}"));
                None
            }
        };
        let mut sdk_count = 0;
        let sdk = match sdk_result {
            Ok((client, found)) => {
                sdk_count = found.len();
                // Do not expose a second writer for the same motherboard.
                devices.extend(
                    found
                        .into_iter()
                        .filter(|d| native.is_none() || d.kind != 0),
                );
                Some(client)
            }
            Err(e) => {
                messages.push(format!("OpenRGB：{e}"));
                None
            }
        };
        if native.is_none() && sdk.is_none() {
            return Err(io::Error::other(messages.join("；")));
        }
        Ok((
            Self {
                native,
                sdk,
                sdk_count,
            },
            devices,
        ))
    }

    pub fn poll(&mut self) -> io::Result<()> {
        if let Some(sdk) = &mut self.sdk {
            sdk.poll()?;
        }
        Ok(())
    }
    pub fn heartbeat(&mut self) -> io::Result<()> {
        if let Some(board) = &self.native {
            board.poll()?;
        }
        if let Some(sdk) = &mut self.sdk
            && sdk.count()? as usize != self.sdk_count
        {
            return Err(io::Error::other("OpenRGB 裝置清單已變更"));
        }
        Ok(())
    }
    pub fn custom(&mut self, id: u32) -> io::Result<()> {
        if id == msi::DEVICE_ID {
            return self.board()?.enable();
        }
        self.sdk()?.custom(id)
    }
    pub fn colors(&mut self, id: u32, colors: &[[u8; 3]]) -> io::Result<()> {
        if id == msi::DEVICE_ID {
            return self.board()?.colors(colors);
        }
        self.sdk()?.colors(id, colors)
    }
    fn board(&mut self) -> io::Result<&mut MsiBoard> {
        self.native
            .as_mut()
            .ok_or_else(|| io::Error::other("主機板已離線"))
    }
    fn sdk(&mut self) -> io::Result<&mut Client> {
        self.sdk
            .as_mut()
            .ok_or_else(|| io::Error::other("OpenRGB 已離線"))
    }
}
