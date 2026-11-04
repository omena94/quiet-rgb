#[path = "../src/msi.rs"]
mod msi;

fn main() -> std::io::Result<()> {
    match msi::MsiBoard::discover()? {
        Some(mut board) => {
            println!(
                "MS-7E51 USB controller found; configuration read OK; id={:#x}",
                msi::DEVICE_ID
            );
            board.poll()?;
            if std::env::args().any(|s| s == "--off") {
                board.colors(&vec![[0, 0, 0]; msi::LEDS])?;
                println!("Sent OFF to JAF + JARGB1/2/3.");
            }
            if std::env::args().any(|s| s == "--dim") {
                board.colors(&vec![[5, 9, 13]; msi::LEDS])?;
                println!("Sent low-brightness blue to JAF + JARGB1/2/3 (4 x 240 slots).");
            }
        }
        None => return Err(std::io::Error::other("MS-7E51 controller not found")),
    }
    Ok(())
}
