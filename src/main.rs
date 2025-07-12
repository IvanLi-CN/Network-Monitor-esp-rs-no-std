#![no_std]
#![no_main]
#![allow(incomplete_features)]


use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
// use fugit::rate::ExtU32; // Will use Hz instead
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
// use esp_hal::dma::Dma; // Not available in 1.0.0-beta.1
// use esp_hal::dma::DmaPriority; // Not needed in 1.0.0-beta.1
use esp_hal::dma_buffers;
use esp_hal::gpio::{Io, Output};
use esp_hal::ledc::{self, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::rng::Rng;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::spi::master::Spi;
use esp_println::println;
// use esp_wifi::wifi::WifiDevice; // Not needed
use st7735::ST7735;

use embassy_net::{Config, StackResources};
mod bus;
mod display;
mod udp_client;
mod wifi;
use wifi::{connection, get_ip_addr, net_task};

use esp_backtrace as _;

use crate::udp_client::receiving_net_speed;

extern crate alloc;
use esp_alloc as _;

// ESP-IDF App Descriptor
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::println!("Init main");

    esp_alloc::heap_allocator!(size: 72 * 1024);

    // Basic stuff

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let _io = Io::new(peripherals.IO_MUX);

    let systimer = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    // Wi-Fi

    static INIT: static_cell::StaticCell<esp_wifi::EspWifiController<'static>> = static_cell::StaticCell::new();
    let init = INIT.init(esp_wifi::init(
        timg0.timer0,
        Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap());
    let wifi = peripherals.WIFI;
    let (controller, interfaces) = esp_wifi::wifi::new(init, wifi).unwrap();
    let wifi_device = interfaces.sta;
    let config = Config::dhcpv4(Default::default());
    let seed = 1234; // very random, very secure seed

    // Init network stack
    static STACK_RESOURCES: static_cell::StaticCell<StackResources<3>> = static_cell::StaticCell::new();
    let stack_resources = STACK_RESOURCES.init(StackResources::<3>::new());

    static STACK: static_cell::StaticCell<embassy_net::Stack<'static>> = static_cell::StaticCell::new();
    static RUNNER: static_cell::StaticCell<embassy_net::Runner<'static, esp_wifi::wifi::WifiDevice<'static>>> = static_cell::StaticCell::new();
    let (stack, runner) = embassy_net::new(
        wifi_device,
        config,
        stack_resources,
        seed
    );
    let stack = STACK.init(stack);
    let runner = RUNNER.init(runner);

    // DMA

    let _dma_channel = peripherals.DMA_CH0;

    // SPI

    let sda = peripherals.GPIO5;
    let sck = peripherals.GPIO6;
    let (_rx_buffer, _rx_descriptors, _tx_buffer, _tx_descriptors) = dma_buffers!(32000, 1024);

    let spi_config = esp_hal::spi::master::Config::default()
        .with_frequency(esp_hal::time::Rate::from_hz(26_000_000)); // 26 MHz for fast SPI
    let spi = Spi::new(peripherals.SPI2, spi_config)
        .unwrap()
        .with_sck(sck)
        .with_mosi(sda)
        .into_async();
    let spi_mutex = Mutex::new(spi);
    static SPI: static_cell::StaticCell<Mutex<NoopRawMutex, esp_hal::spi::master::Spi<'static, esp_hal::Async>>> = static_cell::StaticCell::new();
    let spi = SPI.init(spi_mutex);

    // Display

    let dc = Output::new(peripherals.GPIO7, esp_hal::gpio::Level::High, esp_hal::gpio::OutputConfig::default());
    let rst = Output::new(peripherals.GPIO8, esp_hal::gpio::Level::High, esp_hal::gpio::OutputConfig::default());
    let lcd_cs = Output::new(peripherals.GPIO10, esp_hal::gpio::Level::High, esp_hal::gpio::OutputConfig::default());
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
    static DISPLAY: static_cell::StaticCell<display::DisplayST7735> = static_cell::StaticCell::new();
    let display = DISPLAY.init(display);

    let mut ledc = Ledc::new(peripherals.LEDC);

    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut lstimer0 = ledc.timer::<LowSpeed>(ledc::timer::Number::Timer1);

    lstimer0
        .configure(ledc::timer::config::Config {
            duty: ledc::timer::config::Duty::Duty5Bit,
            clock_source: ledc::timer::LSClockSource::APBClk,
            frequency: esp_hal::time::Rate::from_hz(512_000),
        })
        .unwrap();

    let mut channel0 = ledc.channel(ledc::channel::Number::Channel0, peripherals.GPIO4);
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
    spawner.spawn(net_task(runner)).ok();
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
