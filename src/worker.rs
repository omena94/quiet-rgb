use crate::{backend::Backend, effect::Settings, openrgb::Device};
use std::{
    collections::HashSet,
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
pub struct Desired {
    pub connection: u64,
    pub port: u16,
    pub connected: bool,
    pub active: bool,
    pub selected: Vec<u32>,
    pub settings: Settings,
    pub quit: bool,
}

pub enum Event {
    Connected(Vec<Device>),
    Status(String),
    Error(String),
}

pub struct Worker {
    pub desired: Arc<Mutex<Desired>>,
    pub events: Receiver<Event>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn new() -> Self {
        let desired = Arc::new(Mutex::new(Desired::default()));
        let shared = desired.clone();
        let (tx, events) = mpsc::channel();
        let handle = Some(thread::spawn(move || run(shared, tx)));
        Self {
            desired,
            events,
            handle,
        }
    }
    pub fn shutdown(&mut self) {
        self.desired.lock().unwrap().quit = true;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn release(
    client: &mut Backend,
    devices: &[Device],
    owned: &mut HashSet<u32>,
    off: bool,
) -> io::Result<()> {
    // Discard stale indexes instead of writing after a hardware-list change.
    client.poll()?;
    for device in devices.iter().filter(|d| owned.contains(&d.id)) {
        if off {
            client.colors(device.id, &vec![[0, 0, 0]; device.leds])?;
        }
    }
    owned.clear();
    Ok(())
}

fn run(shared: Arc<Mutex<Desired>>, tx: Sender<Event>) {
    let mut client: Option<Backend> = None;
    let mut devices = Vec::new();
    let mut owned = HashSet::new();
    let mut generation = 0;
    let mut previous: Option<(Settings, Vec<u32>)> = None;
    let mut heartbeat = Instant::now();
    let mut start = Instant::now();
    loop {
        let desired = shared.lock().unwrap().clone();
        if desired.quit {
            if let Some(c) = client.as_mut() {
                let _ = release(c, &devices, &mut owned, desired.settings.off_on_exit);
            }
            return;
        }
        if desired.connection != generation {
            if let Some(c) = client.as_mut()
                && let Err(e) = release(c, &devices, &mut owned, true)
            {
                let _ = tx.send(Event::Error(format!("停止裝置時連線中斷：{e}")));
            }
            client = None;
            owned.clear();
            generation = desired.connection;
            previous = None;
            if desired.connected {
                let _ = tx.send(Event::Status("正在偵測主機板 USB 與 OpenRGB…".into()));
                match Backend::connect(desired.port) {
                    Ok((c, found)) => {
                        devices = found;
                        client = Some(c);
                        heartbeat = Instant::now();
                        let _ = tx.send(Event::Connected(devices.clone()));
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Error(format!(
                            "連線失敗：{e}。請啟動 OpenRGB SDK 伺服器。"
                        )));
                    }
                }
            } else {
                let _ = tx.send(Event::Status("已中斷連線，回到預覽".into()));
            }
        }
        if let Some(c) = client.as_mut() {
            let result = (|| -> io::Result<()> {
                c.poll()?;
                if heartbeat.elapsed() >= Duration::from_secs(2) {
                    c.heartbeat()?;
                    heartbeat = Instant::now();
                }
                if !desired.active {
                    if !owned.is_empty() {
                        release(c, &devices, &mut owned, true)?;
                        let _ = tx.send(Event::Status("已停止並熄燈；目前僅預覽".into()));
                    }
                    previous = None;
                    return Ok(());
                }
                let changed = previous
                    .as_ref()
                    .is_none_or(|(s, ids)| *s != desired.settings || *ids != desired.selected);
                if changed {
                    start = Instant::now();
                }
                for d in &devices {
                    let selected = desired.selected.contains(&d.id) && d.controllable;
                    if !selected && owned.remove(&d.id) {
                        c.colors(d.id, &vec![[0, 0, 0]; d.leds])?;
                    }
                    if selected {
                        let acquired = owned.insert(d.id);
                        if acquired {
                            c.custom(d.id)?;
                        }
                        if acquired || changed || desired.settings.effect.animated() {
                            c.colors(
                                d.id,
                                &desired
                                    .settings
                                    .colors(d.leds, start.elapsed().as_secs_f32()),
                            )?;
                        }
                    }
                }
                if changed {
                    let _ = tx.send(Event::Status(format!(
                        "控制中 · {} 個裝置 · {}",
                        owned.len(),
                        desired.settings.effect.label()
                    )));
                    previous = Some((desired.settings.clone(), desired.selected.clone()));
                }
                Ok(())
            })();
            if let Err(e) = result {
                client = None;
                owned.clear();
                previous = None;
                let _ = tx.send(Event::Error(format!(
                    "連線已停止：{e}。實際燈光可能停在最後狀態，請重新連線。"
                )));
            }
        }
        thread::sleep(Duration::from_millis(33));
    }
}
