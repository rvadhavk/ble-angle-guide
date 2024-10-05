# BLE Angle Guide
<img src="https://github.com/rvadhavk/ble-angle-guide/blob/main/screenshot.png?raw=true" width="400" alt="screnshot of the gui showing angle readings"/>
This project is my attempt to create a BLE knife sharpening angle guide/sensor that magnetically clips onto the side of your knife. The UI is a webpage which talks to the sensor using WebBluetooth. The hardware is a custom PCB based around a Panasonic nRF52840 module. The firmware is implemented in Rust using [Embassy](https://github.com/embassy-rs/embassy).  It's my first foray into embedded systems, so the PCB design is probably terrible.

## Project Structure
- [hardware/](./hardware): KiCad files for the PCB
- [firmware/](./firmware): Rust firmware for the nRF52840 module
- [ui/](./ui): Web-based UI (just an index.html file)

## Building the Firmware
The board has 1.27mm/0.05" pitch 2x5 pads which can be connected to using something like <https://www.adafruit.com/product/5434>.  For anyone wanting to develop something similar, a 10 pin connector is unnecessary; all you need is 4 pins: SWCLK, SWDIO, GND, and VCC.

To build and debug, run `cargo run --release` from the `firmware` directory.  Note: the CPU seems to hardfault after some time if running without the `--release` flag.  I haven't figured out why yet; I'm guessing it has to do with a stack overflow.

To flash the device, run `sh flash.sh` from the `firmware` directory.

## UI
The UI is just a single [index.html](./ui/index.html) file.  WebBluetooth requires an https connection, so [development-server.py](./ui/development-server.py) is included for running a simple HTTP server with TLS support.  It requires that `mkcert` be installed or that you manually generate `cert.pem` and `key.pem`.
