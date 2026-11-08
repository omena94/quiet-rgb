# Third-party components

Quiet RGB is GPL-2.0-or-later. The MSI USB backend in `src/msi.rs` adapts the
761-byte motherboard protocol implemented by rom4ster (2025) in OpenRGB,
GPL-2.0-or-later. The associated upstream implementation credits SignalRGB
for the direct-mode protocol. Original copyright and license notices remain
in the linked source files.

- [MSIMotherboard761Controller.cpp](https://github.com/CalcProgrammer1/OpenRGB/blob/81bbe18a84c2e507006f19dd252e397e40a56bfe/Controllers/MSIMotherboardController/MSIMotherboard761Controller/MSIMotherboard761Controller.cpp)
- [MSIMotherboardCommon.h](https://github.com/CalcProgrammer1/OpenRGB/blob/81bbe18a84c2e507006f19dd252e397e40a56bfe/Controllers/MSIMotherboardController/MSIMotherboardCommon.h)
- [OpenRGB SDK protocol](https://github.com/CalcProgrammer1/OpenRGB/blob/81bbe18a84c2e507006f19dd252e397e40a56bfe/Documentation/OpenRGBSDK.md)

Core Rust dependencies: eframe/egui (MIT OR Apache-2.0), hidapi (MIT), serde
(MIT OR Apache-2.0). Exact versions and transitive dependencies are recorded
in Cargo.lock. OpenRGB and PawnIO are separate optional tools, not linked into
QuietRGB.exe. OpenRGB test downloads are excluded from source distribution.

When distributing the executable, provide this project's corresponding source,
including Cargo.toml, Cargo.lock, src/, and build scripts under its GPL license.
