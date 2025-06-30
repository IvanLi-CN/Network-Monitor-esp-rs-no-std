#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(const_refs_to_static)]
#![feature(const_maybe_uninit_write)]
#![feature(const_mut_refs)]
#![feature(int_roundings)]
#![feature(impl_trait_in_assoc_type)]
#![allow(incomplete_features)]


use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::dma::Dma;
use esp_hal::dma::DmaPriority;
use esp_hal::dma_buffers;
use esp_hal::gpio::{Io, Output};
use esp_hal::ledc::{self, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::rng::Rng;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    prelude::*,
    spi::{master::{Spi, dma::SpiDma, prelude::*}, SpiMode},
};
use esp_println::println;
use esp_wifi::wifi::WifiStaDevice;
use esp_wifi::EspWifiInitFor;
use st7735::ST7735;

use embassy_net::{Config, Stack, StackResources};
mod bus;
mod display;
mod udp_client;
mod wifi;
use wifi::{connection, get_ip_addr, net_task};

use esp_backtrace as _;

use crate::udp_client::receiving_net_speed;

extern crate alloc;
use esp_alloc as _;

#[main]
async fn main(spawner: Spawner) {
    esp_println::println!("Init main");

    esp_alloc::heap_allocator!(72 * 1024);

    // Basic stuff

    let peripherals = esp_hal::peripherals::Peripherals::take();
    let system = esp_hal::system::SystemControl::new(peripherals.SYSTEM);
    let clocks = esp_hal::clock::ClockControl::max(system.clock_control).freeze();

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    let alarm0: esp_hal::timer::ErasedTimer = systimer.alarm0.into();
    static TIMERS: static_cell::StaticCell<[esp_hal::timer::OneShotTimer<esp_hal::timer::ErasedTimer>; 1]> = static_cell::StaticCell::new();
    let timers = TIMERS.init([esp_hal::timer::OneShotTimer::new(alarm0)]);
    esp_hal_embassy::init(&clocks, timers);
    let timg0 = TimerGroup::new(peripherals.TIMG0, &clocks, None);

    // Wi-Fi

    let timer0: esp_hal::timer::ErasedTimer = timg0.timer0.into();
    let init = esp_wifi::initialize(
        EspWifiInitFor::Wifi,
        esp_hal::timer::PeriodicTimer::new(timer0),
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
        &clocks,
    )
    .unwrap();
    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();
    let config = Config::dhcpv4(Default::default());
    let seed = 1234; // very random, very secure seed

    // Init network stack
    static STACK_RESOURCES: static_cell::StaticCell<StackResources<3>> = static_cell::StaticCell::new();
    let stack_resources = STACK_RESOURCES.init(StackResources::<3>::new());

    static STACK: static_cell::StaticCell<Stack<esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>>> = static_cell::StaticCell::new();
    let stack = STACK.init(Stack::new(
        wifi_interface,
        config,
        stack_resources,
        seed
    ));

    // DMA

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.channel0;

    // SPI

    let sda = io.pins.gpio5;
    let sck = io.pins.gpio6;
    let (_rx_buffer, rx_descriptors, _tx_buffer, tx_descriptors) = dma_buffers!(32000, 1024);

    let spi = Spi::new(peripherals.SPI2, 40u32.MHz(), SpiMode::Mode0, &clocks)
        .with_sck(sck)
        .with_mosi(sda)
        .with_dma(
            dma_channel.configure_for_async(false, DmaPriority::Priority0),
            rx_descriptors,
            tx_descriptors,
        );
    let spi_mutex: Mutex<NoopRawMutex, _> = Mutex::new(spi);
    static SPI: static_cell::StaticCell<Mutex<NoopRawMutex, SpiDma<'static, esp_hal::peripherals::SPI2, esp_hal::dma::Channel0, esp_hal::spi::FullDuplexMode, esp_hal::Async>>> = static_cell::StaticCell::new();
    let spi = SPI.init(spi_mutex);

    // Display

    let dc = Output::new(io.pins.gpio7, esp_hal::gpio::Level::High);
    let rst = Output::new(io.pins.gpio8, esp_hal::gpio::Level::High);
    let lcd_cs = Output::new(io.pins.gpio10, esp_hal::gpio::Level::High);
    let spi_dev = SpiDevice::new(spi, lcd_cs);

    let width = 160;
    let height = 80;

    println!("lcd init...");
    let display = ST7735::new(
        spi_dev,
        dc,
        rst,
        st7735::Config {
            rgb: false,
            inverted: false,
            orientation: st7735::Orientation::Landscape,
        },
        width,
        height,
    );
    static DISPLAY: static_cell::StaticCell<ST7735<embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice<'static, NoopRawMutex, SpiDma<'static, esp_hal::peripherals::SPI2, esp_hal::dma::Channel0, esp_hal::spi::FullDuplexMode, esp_hal::Async>, esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<10>>>, esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<7>>, esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<8>>>> = static_cell::StaticCell::new();
    let display = DISPLAY.init(display);

    let mut ledc = Ledc::new(peripherals.LEDC, &clocks);

    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut lstimer0 = ledc.get_timer::<LowSpeed>(ledc::timer::Number::Timer1);

    lstimer0
        .configure(ledc::timer::config::Config {
            duty: ledc::timer::config::Duty::Duty5Bit,
            clock_source: ledc::timer::LSClockSource::APBClk,
            frequency: 512.kHz(),
        })
        .unwrap();

    let mut channel0 = ledc.get_channel(ledc::channel::Number::Channel0, io.pins.gpio4);
    channel0
        .configure(ledc::channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            pin_config: ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();

    spawner.spawn(display::init_display(display)).ok();
    // spawner.spawn(blink(blink_led)).ok();
    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(stack)).ok();
    spawner.spawn(get_ip_addr(stack)).ok();
    spawner.spawn(receiving_net_speed(stack)).ok();

    Timer::after(Duration::from_millis(500)).await;
    // blk 0 to 50 fade
    for i in 0..50 {
        channel0.set_duty(i).unwrap();
        Timer::after(Duration::from_millis((60 - i) as u64)).await;
    }

    loop {
        Timer::after(Duration::from_millis(1000)).await;
    }
}
