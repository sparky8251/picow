//! This example test the RP Pico W on board LED.
//!
//! It does not work with the RP Pico board. See blinky.rs.

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use bmp390::i2c::BMP390;
use bmp390::{PowerConfig, PowerMode};
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Delay, Duration, Timer};
use static_cell::make_static;

use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

#[embassy_executor::task]
async fn wifi_task(runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let fw = include_bytes!("cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    let sda = p.PIN_14;
    let scl = p.PIN_15;
    let i2c_bus = i2c::I2c::new_blocking(p.I2C1, scl, sda, Config::default());

    let state = make_static!(cyw43::State::new());
    let (_net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let mut bmp390 = BMP390::new_primary(i2c_bus);
    let bmp390id = bmp390.init(&mut Delay, None).unwrap();
    info!("Chip ID and Rev is ID: {:x}, Rev: {:x}", bmp390id.chip_id, bmp390id.rev_id);

    let delay = Duration::from_secs(25);
    loop {
        control.gpio_set(0, true).await;
        bmp390.set_power_config(&PowerConfig {
            pressure_enable: true,
            temperature_enable: false,
            power_mode: PowerMode::Forced
        }).unwrap();
        Timer::after(delay).await;

        control.gpio_set(0, false).await;
        let measurement = bmp390.take_measurement().unwrap();
        info!("Temp {}, Press {}", measurement.temp, measurement.press);
        Timer::after(delay).await;
    }
}
