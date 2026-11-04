//! A small, bounded OpenRGB SDK client. Negotiate v1 even with newer servers.
//! No USB/SMBus writes or controller resizing are performed by this application.
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

const VERSION: u32 = 1;
const MAX_PACKET: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Device {
    pub id: u32,
    pub kind: u32,
    pub name: String,
    pub vendor: String,
    pub leds: usize,
    pub modes: Vec<String>,
    pub controllable: bool,
}

impl Device {
    pub fn msi_board() -> Self {
        Self {
            id: crate::msi::DEVICE_ID,
            kind: 0,
            name: "MSI MAG X870 TOMAHAWK WIFI".into(),
            vendor: "原生 USB · MS-7E51".into(),
            leds: crate::msi::LEDS,
            modes: vec!["JAF + JARGB 1 / 2 / 3".into()],
            controllable: true,
        }
    }
}

pub struct Client {
    stream: TcpStream,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl Client {
    pub fn connect(port: u16) -> io::Result<(Self, Vec<Device>)> {
        let address = SocketAddr::from(([127, 0, 0, 1], port));
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.set_nodelay(true)?;
        let mut client = Self { stream };
        let version = client.request(0, 40, &VERSION.to_le_bytes())?;
        if Reader::new(&version).u32()? < VERSION {
            return Err(invalid("請使用 OpenRGB 0.5 或更新版本"));
        }
        client.send(0, 50, b"Quiet RGB\0")?;
        let count = client.count()?;
        let mut devices = Vec::new();
        for id in 0..count {
            let data = client.request(id, 1, &VERSION.to_le_bytes())?;
            devices.push(parse_device(id, &data)?);
        }
        Ok((client, devices))
    }

    fn send(&mut self, device: u32, command: u32, data: &[u8]) -> io::Result<()> {
        let mut packet = Vec::with_capacity(16 + data.len());
        packet.extend_from_slice(b"ORGB");
        packet.extend_from_slice(&device.to_le_bytes());
        packet.extend_from_slice(&command.to_le_bytes());
        packet.extend_from_slice(&(data.len() as u32).to_le_bytes());
        packet.extend_from_slice(data);
        self.stream.write_all(&packet)
    }

    fn receive(&mut self) -> io::Result<(u32, u32, Vec<u8>)> {
        let mut header = [0; 16];
        self.stream.read_exact(&mut header)?;
        if &header[..4] != b"ORGB" {
            return Err(invalid("OpenRGB 封包識別錯誤"));
        }
        let mut r = Reader::new(&header[4..]);
        let device = r.u32()?;
        let command = r.u32()?;
        let length = r.u32()? as usize;
        if length > MAX_PACKET {
            return Err(invalid("OpenRGB 封包超出大小上限"));
        }
        let mut data = vec![0; length];
        self.stream.read_exact(&mut data)?;
        Ok((device, command, data))
    }

    fn request(&mut self, device: u32, command: u32, data: &[u8]) -> io::Result<Vec<u8>> {
        self.send(device, command, data)?;
        for _ in 0..32 {
            let (id, cmd, data) = self.receive()?;
            if cmd == 100 {
                return Err(invalid("裝置清單已變更，請重新連線後選取装置"));
            }
            if id == device && cmd == command {
                return Ok(data);
            }
        }
        Err(invalid("OpenRGB 回覆與請求不符"))
    }

    pub fn count(&mut self) -> io::Result<u32> {
        let data = self.request(0, 0, &[])?;
        let count = Reader::new(&data).u32()?;
        if count > 256 {
            return Err(invalid("裝置數超出上限"));
        }
        Ok(count)
    }

    // Check asynchronous device-list invalidation before using indexed device IDs.
    pub fn poll(&mut self) -> io::Result<()> {
        self.stream.set_nonblocking(true)?;
        let result = self.stream.peek(&mut [0; 1]);
        self.stream.set_nonblocking(false)?;
        match result {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "OpenRGB 已關閉連線",
            )),
            Ok(_) => {
                let (_, command, _) = self.receive()?;
                if command == 100 {
                    Err(invalid("裝置清單已變更，請重新連線"))
                } else {
                    Ok(())
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn custom(&mut self, id: u32) -> io::Result<()> {
        self.send(id, 1100, &[])
    }

    pub fn colors(&mut self, id: u32, colors: &[[u8; 3]]) -> io::Result<()> {
        if colors.len() > u16::MAX as usize {
            return Err(invalid("LED 數量超出上限"));
        }
        let length = 6 + colors.len() * 4;
        let mut data = Vec::with_capacity(length);
        data.extend_from_slice(&(length as u32).to_le_bytes());
        data.extend_from_slice(&(colors.len() as u16).to_le_bytes());
        for c in colors {
            data.extend_from_slice(&[c[0], c[1], c[2], 0]);
        }
        self.send(id, 1050, &data)
    }
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or_else(|| invalid("資料長度溢位"))?;
        let data = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| invalid("OpenRGB 資料不完整"))?;
        self.pos = end;
        Ok(data)
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> io::Result<String> {
        let size = self.u16()? as usize;
        let bytes = self.take(size)?;
        Ok(String::from_utf8_lossy(bytes.strip_suffix(&[0]).unwrap_or(bytes)).into_owned())
    }
}

fn parse_device(id: u32, data: &[u8]) -> io::Result<Device> {
    let mut r = Reader::new(data);
    if r.u32()? as usize != data.len() {
        return Err(invalid("裝置資料長度不符"));
    }
    let kind = r.u32()?;
    let name = r.string()?;
    let vendor = r.string()?;
    for _ in 0..4 {
        r.string()?;
    } // description, version, serial, location
    let mode_count = r.u16()?;
    let active = r.u32()?;
    let mut modes = Vec::new();
    let mut active_found = false;
    let mut controllable = false;
    for index in 0..mode_count {
        let name = r.string()?;
        r.take(32)?; // value, flags, speed bounds, color bounds, speed, direction
        let color_mode = r.u32()?;
        let count = r.u16()? as usize;
        r.take(count * 4)?;
        controllable |=
            matches!(name.as_str(), "Direct" | "Custom" | "Static") && matches!(color_mode, 1 | 2);
        if index as u32 == active {
            active_found = true;
        }
        modes.push(name);
    }
    let zones = r.u16()?;
    for _ in 0..zones {
        r.string()?;
        r.take(16)?;
        let matrix_size = r.u16()? as usize;
        r.take(matrix_size)?;
    }
    let leds = r.u16()? as usize;
    for _ in 0..leds {
        r.string()?;
        r.u32()?;
    }
    let count = r.u16()? as usize;
    r.take(count * 4)?;
    if r.pos != data.len() || count != leds {
        return Err(invalid("裝置 LED 資料不一致"));
    }
    controllable &= leds > 0 && active_found;
    Ok(Device {
        id,
        kind,
        name,
        vendor,
        leds,
        modes,
        controllable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend_from_slice(&((s.len() + 1) as u16).to_le_bytes());
        out.extend_from_slice(s.as_bytes());
        out.push(0);
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 8];
        for s in ["Test board", "MSI", "Description", "1", "serial", "USB"] {
            string(&mut b, s);
        }
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        string(&mut b, "Direct");
        b.extend_from_slice(&[0; 32]);
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes()); // zones
        b.extend_from_slice(&1u16.to_le_bytes());
        string(&mut b, "LED");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&[12, 34, 56, 0]);
        let len = b.len() as u32;
        b[..4].copy_from_slice(&len.to_le_bytes());
        b
    }
    #[test]
    fn parses_device_and_checks_control_capabilities() {
        let d = parse_device(2, &fixture()).unwrap();
        assert_eq!(d.name, "Test board");
        assert!(d.controllable);
        assert_eq!(d.leds, 1);
    }
    #[test]
    fn rejects_every_truncation_without_panicking() {
        let f = fixture();
        for len in 0..f.len() {
            assert!(parse_device(0, &f[..len]).is_err());
        }
        let mut f = f;
        f[0] = 0;
        assert!(parse_device(0, &f).is_err());
    }
    #[test]
    fn loopback_checks_negotiation_and_wire_color_order() {
        use std::net::TcpListener;
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut peer = Client { stream };
            let (_, cmd, data) = peer.receive().unwrap();
            assert_eq!(cmd, 40);
            assert_eq!(data, 1u32.to_le_bytes());
            peer.send(0, 40, &6u32.to_le_bytes()).unwrap();
            assert_eq!(peer.receive().unwrap().1, 50);
            assert_eq!(peer.receive().unwrap().1, 0);
            peer.send(0, 0, &1u32.to_le_bytes()).unwrap();
            assert_eq!(peer.receive().unwrap().1, 1);
            peer.send(0, 1, &fixture()).unwrap();
            assert_eq!(peer.receive().unwrap().1, 1100);
            let (id, cmd, data) = peer.receive().unwrap();
            assert_eq!((id, cmd), (0, 1050));
            assert_eq!(data, [10, 0, 0, 0, 1, 0, 9, 8, 7, 0]);
            peer.send(0, 100, &[]).unwrap();
            assert_eq!(peer.receive().unwrap().1, 0);
        });
        let (mut client, devices) = Client::connect(port).unwrap();
        assert_eq!(devices.len(), 1);
        client.custom(0).unwrap();
        client.colors(0, &[[9, 8, 7]]).unwrap();
        assert!(client.count().is_err());
        server.join().unwrap();
    }
}
