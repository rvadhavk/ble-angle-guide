#![no_std]
#![no_main]

use core;
use core::mem;
use defmt::*;
use defmt_rtt as _; // global logger
use embassy_executor::Spawner;
use embassy_nrf as _; // time driver
use embassy_nrf::interrupt::{self, InterruptExt, Priority};
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::{bind_interrupts, peripherals};
use embassy_time::{Duration, Timer};
use embedded_hal::i2c::I2c;
use futures::future::select;
use futures::pin_mut;
use nrf_softdevice::ble::advertisement_builder::{Flag, LegacyAdvertisementBuilder, ServiceList};
use nrf_softdevice::ble::{
    gatt_server,
    peripheral::{self, advertise_connectable},
};
use nrf_softdevice::{raw, Softdevice};
use panic_probe as _;

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}

// Needed for TWIM for some reason
bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

struct AccelerometerDriver<I2C: I2c> {
    i2c: I2C,
}

impl<I2C: I2c> AccelerometerDriver<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    const ADDRESS: u8 = 0x4c;

    pub fn reset(&mut self) -> Result<(), I2C::Error> {
        self.sleep()?;
        self.configure()?;
        self.wake()
    }

    pub fn sleep(&mut self) -> Result<(), I2C::Error> {
        self.i2c.write(Self::ADDRESS, &[0x07, 0x00])
    }

    pub fn wake(&mut self) -> Result<(), I2C::Error> {
        self.i2c.write(Self::ADDRESS, &[0x07, 0x01])
    }

    pub fn configure(&mut self) -> Result<(), I2C::Error> {
        // setup low pass filter
        self.i2c.write(Self::ADDRESS, &[0x20, 0b0000_1101])?;
        // set internal data rate
        self.i2c.write(Self::ADDRESS, &[0x08, 0x00])?;
        // set decimation
        self.i2c.write(Self::ADDRESS, &[0x30, 0b00])
    }

    pub fn read_xyz(&mut self) -> Result<[u8; 6], I2C::Error> {
        let mut buf = [0u8; 6];
        self.i2c.write_read(Self::ADDRESS, &[0x0d], &mut buf)?;
        Ok(buf)
    }
}

#[nrf_softdevice::gatt_service(uuid = "b3f787c6-77c9-49c1-bd93-3692eb94d2ca")]
struct AccelerometerService {
    #[characteristic(uuid = "6ec6e014-9fe0-4e14-97a1-bbf221d19dec", read, notify)]
    xyz: [u8; 6],
}

#[nrf_softdevice::gatt_server]
struct Server {
    accelerometer: AccelerometerService,
}

const DEVICE_NAME: &str = "Inclinometer";

async fn run_bluetooth(
    sd: &'static Softdevice,
    server: &Server,
    accelerometer: &mut AccelerometerDriver<impl I2c>,
) {
    let advertisement = peripheral::ConnectableAdvertisement::ScannableUndirected {
        adv_data: &LegacyAdvertisementBuilder::new()
            .flags(&[Flag::GeneralDiscovery, Flag::LE_Only])
            .full_name(DEVICE_NAME)
            .build(),
        scan_data: &LegacyAdvertisementBuilder::new()
            .services_128(
                ServiceList::Complete,
                &[0xb3f787c6_77c9_49c1_bd93_3692eb94d2ca_u128.to_le_bytes()],
            )
            .build(),
    };
    info!("Advertising...");
    let conn = unwrap!(advertise_connectable(sd, advertisement, &Default::default()).await);
    info!("Connected!");
    accelerometer.reset().unwrap();
    let accelerometer_read_task = async {
        loop {
            let xyz = match accelerometer.read_xyz() {
                Ok(xyz) => xyz,
                Err(_) => {
                    error!("Failed to read xyz");
                    [0; 6]
                }
            };
            if let Err(e) = server.accelerometer.xyz_set(&xyz) {
                error!("Failed to set xyz: {:?}", e);
            }
            if let Err(e) = server.accelerometer.xyz_notify(&conn, &xyz) {
                error!("Failed to notify xyz: {:?}", e);
            }
            Timer::after(Duration::from_millis(33)).await;
        }
    };

    let server_task = gatt_server::run(&conn, server, |_| {});
    pin_mut!(accelerometer_read_task);
    pin_mut!(server_task);
    let _ = select(accelerometer_read_task, server_task).await;
    accelerometer.sleep().unwrap();
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Starting...");


    let peripherals = embassy_nrf::init({
        let mut config = embassy_nrf::config::Config::default();
        // These need to be lower priority than softdevice interrupts
        config.gpiote_interrupt_priority = Priority::P2;
        config.time_interrupt_priority = Priority::P2;
        config
    });
    let sd = Softdevice::enable(&nrf_softdevice::Config {
        gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
            p_value: DEVICE_NAME.as_bytes().as_ptr() as _,
            current_len: DEVICE_NAME.len() as _,
            max_len: DEVICE_NAME.len() as _,
            write_perm: unsafe { mem::zeroed() },
            _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                raw::BLE_GATTS_VLOC_STACK as u8,
            ),
        }),
        ..Default::default()
    });

    info!("Initializing TWI...");
    let (twi0, sda, scl) = (peripherals.TWISPI0, peripherals.P0_06, peripherals.P0_27);
    interrupt::SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0.set_priority(Priority::P3);
    let twi = Twim::new(twi0, Irqs, sda, scl, {
        let mut config = twim::Config::default();
        config.scl_pullup = true;
        config.sda_pullup = true;
        config
    });

    let mut accelerometer = AccelerometerDriver::new(twi);
    let server = unwrap!(Server::new(sd));
    unwrap!(spawner.spawn(softdevice_task(sd)));
    loop {
        run_bluetooth(sd, &server, &mut accelerometer).await;
    }
}
