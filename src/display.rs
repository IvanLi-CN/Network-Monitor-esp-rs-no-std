use crate::bus::{NetSpeed, WiFiConnectStatus, NET_SPEED, WIFI_CONNECT_STATUS};
use core::future::Future;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_futures::select::select;
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Instant, Ticker};
use embedded_graphics::image::{Image, ImageRaw, ImageRawLE};
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::pixelcolor::{Rgb565, Rgb888};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyleBuilder, Rectangle, RoundedRectangle};
use embedded_graphics::text::TextStyle;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};

use esp_hal::peripherals::SPI2;
use esp_hal::spi::master::dma::SpiDma;
use esp_hal::spi::FullDuplexMode;
use esp_hal::Async;
use heapless::String;
use st7735::ST7735;


pub(crate) type DisplayST7735 = ST7735<
    SpiDevice<
        'static,
        NoopRawMutex,
        SpiDma<'static, SPI2, esp_hal::dma::Channel0, FullDuplexMode, Async>,
        esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<10>>,
    >,
    esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<7>>,
    esp_hal::gpio::Output<'static, esp_hal::gpio::GpioPin<8>>,
>;

#[embassy_executor::task]
pub(crate) async fn init_display(display: &'static mut DisplayST7735) {
    loop {
        let mut gui: GUI<'static> = GUI::new(display);

        gui.init().await;

        static GUI: static_cell::StaticCell<Mutex<CriticalSectionRawMutex, GUI<'static>>> = static_cell::StaticCell::new();
        let gui = GUI.init(Mutex::<CriticalSectionRawMutex, _>::new(gui));

        let mut wifi_status: WiFiConnectStatus;

        loop {
            let wifi_status_guard = WIFI_CONNECT_STATUS.lock().await;
            wifi_status = *wifi_status_guard;
            drop(wifi_status_guard);

            let mut gui = gui.lock().await;
            match wifi_status {
                WiFiConnectStatus::Connecting => gui.wifi_connecting_display().await,
                WiFiConnectStatus::Connected => match gui.network_speed().await {
                    Ok(_) => {}
                    Err(_) => {
                        continue;
                    }
                },
                _ => {}
            };
            drop(gui);
        }
    }
}

enum DisplayPage<'a> {
    Init,
    Connecting(WiFiConnectingPage<'a>),
    NetworkSpeed(NetDataTrafficSpeedPage<'a>),
}

struct GUI<'a> {
    display: &'a mut DisplayST7735,
    page: DisplayPage<'a>,
}

impl<'a> GUI<'a> {
    fn new(display: &'a mut DisplayST7735) -> Self {
        Self {
            display,
            page: DisplayPage::Init,
        }
    }

    pub async fn init(&mut self) {
        self.display.init(&mut Delay).await.unwrap();
        self.display.set_offset(0, 24);
        self.display.clear(Rgb565::BLACK).unwrap();
        self.display.flush().await.unwrap();

        let image_raw: ImageRawLE<Rgb565> =
            ImageRaw::new(include_bytes!("./assets/simple-icons_espressif.raw"), 24);
        let image = Image::new(&image_raw, Point::new(160 - 60, 80 - 24 - 8));
        image.draw(self.display).unwrap();

        let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(
            include_bytes!("./assets/vscode-icons_file-type-rust.raw"),
            24,
        );
        let image = Image::new(&image_raw, Point::new(160 - 24 - 8, 80 - 24 - 8));
        image.draw(self.display).unwrap();

        self.display.flush().await.unwrap();
    }

    pub async fn wifi_connecting_display(&mut self) {
        if !matches!(self.page, DisplayPage::Connecting(_)) {
            self.page = DisplayPage::Connecting(WiFiConnectingPage::new());
        }

        if let DisplayPage::Connecting(ref mut page) = self.page {
            page.frame(&mut self.display).await;
        }
    }

    pub async fn network_speed(&mut self) -> Result<(), ()> {
        if !matches!(self.page, DisplayPage::NetworkSpeed(_)) {
            self.page = DisplayPage::NetworkSpeed(NetDataTrafficSpeedPage::new());
        }

        if let DisplayPage::NetworkSpeed(ref mut page) = self.page {
            page.frame(&mut self.display).await;
        }

        Ok(())
    }
}

trait GUIPageFrame {
    fn frame(&mut self, display: &mut DisplayST7735) -> impl Future<Output = ()>;
}

struct WiFiConnectingPage<'a> {
    animation_frame_index: u8,
    last_draw_time: Instant,
    character_style: MonoTextStyle<'a, Rgb565>,
    text_style: TextStyle,
    string: String<20>,
}

impl<'a> WiFiConnectingPage<'a> {
    pub fn new() -> Self {
        Self {
            animation_frame_index: 0,
            last_draw_time: Instant::MIN,
            character_style: MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
            text_style: TextStyleBuilder::new()
                .baseline(Baseline::Middle)
                .alignment(Alignment::Left)
                .build(),
            string: String::new(),
        }
    }
}

impl<'a> GUIPageFrame for WiFiConnectingPage<'a> {
    async fn frame(&mut self, display: &mut DisplayST7735) {
        if self.last_draw_time.elapsed().as_millis() < 200 {
            return;
        }

        display.clear(Rgb565::BLACK).unwrap();

        self.last_draw_time = Instant::now();

        self.string.clear();
        self.string.push_str("Connecting").unwrap();

        for _ in 0..self.animation_frame_index {
            self.string.push_str(".").unwrap();
        }

        let image_raw: ImageRawLE<Rgb565> = match self.animation_frame_index {
            0 => {
                self.animation_frame_index = 1;
                ImageRaw::new(include_bytes!("./assets/ci_wifi-none.raw"), 32)
            }
            1 => {
                self.animation_frame_index = 2;
                ImageRaw::new(include_bytes!("./assets/ci_wifi-low.raw"), 32)
            }
            2 => {
                self.animation_frame_index = 3;
                ImageRaw::new(include_bytes!("./assets/ci_wifi-medium.raw"), 32)
            }
            _ => {
                self.animation_frame_index = 0;
                ImageRaw::new(include_bytes!("./assets/ci_wifi-high.raw"), 32)
            }
        };
        let image = Image::new(&image_raw, Point::new(0, 24));
        image.draw(display).unwrap();

        Text::with_text_style(
            self.string.as_str(),
            Point::new(30, 40),
            self.character_style,
            self.text_style,
        )
        .draw(display)
        .unwrap();

        display.flush().await.unwrap();
    }
}

struct NetDataTrafficSpeedPage<'a> {
    character_style: MonoTextStyle<'a, Rgb565>,
    text_style: TextStyle,
    prev_speed: NetSpeed,
    str_buff: [u8; 20],
    string: String<20>,
}

impl<'a> NetDataTrafficSpeedPage<'a> {
    pub fn new() -> Self {
        Self {
            character_style: MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE),
            text_style: TextStyleBuilder::new()
                .baseline(Baseline::Bottom)
                .alignment(Alignment::Right)
                .build(),
            prev_speed: NetSpeed::default(),
            str_buff: [0u8; 20],
            string: String::new(),
        }
    }
}

impl<'a> GUIPageFrame for NetDataTrafficSpeedPage<'a> {
    async fn frame(&mut self, display: &mut DisplayST7735) {
        let curr_speed_guard = NET_SPEED.lock().await;
        let curr_speed = *curr_speed_guard;

        self.prev_speed = curr_speed;
        drop(curr_speed_guard);

        // Direct

        {
            let style = PrimitiveStyleBuilder::new()
                // #f94144
                .fill_color(Rgb565::from(Rgb888::new(0xf9, 0x41, 0x44)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(0, 0), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();
            let style = PrimitiveStyleBuilder::new()
                // #277da1
                .fill_color(Rgb565::from(Rgb888::new(0x27, 0x7d, 0xa1)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(81, 0), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();

            // UP

            curr_speed.get_direct_up_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(75, 22),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();

            // DOWN

            curr_speed.get_direct_down_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(155, 22),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();
        }

        // Proxy
        {
            let style = PrimitiveStyleBuilder::new()
                // f3722c
                .fill_color(Rgb565::from(Rgb888::new(0xf3, 0x72, 0x2c)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(0, 28), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();
            let style = PrimitiveStyleBuilder::new()
                // 577590
                .fill_color(Rgb565::from(Rgb888::new(0x57, 0x75, 0x90)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(81, 28), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();

            // UP

            curr_speed.get_proxy_up_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(75, 50),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();

            // DOWN

            curr_speed.get_proxy_down_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(155, 50),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();
        }

        // Bypass
        {
            let style = PrimitiveStyleBuilder::new()
                // f8961e
                .fill_color(Rgb565::from(Rgb888::new(0xf8, 0x96, 0x1e)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(0, 56), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();
            let style = PrimitiveStyleBuilder::new()
                // 4d908e
                .fill_color(Rgb565::from(Rgb888::new(0x4d, 0x90, 0x8e)))
                .build();
            RoundedRectangle::with_equal_corners(
                Rectangle::new(Point::new(81, 56), Size::new(79, 24)),
                Size::new(5, 5),
            )
            .into_styled(style)
            .draw(display)
            .unwrap();

            // UP

            curr_speed.get_bypass_up_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(75, 78),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();

            // DOWN

            curr_speed.get_bypass_down_bps_str(&mut self.string, &mut self.str_buff);

            Text::with_text_style(
                self.string.as_str(),
                Point::new(155, 78),
                self.character_style,
                self.text_style,
            )
            .draw(display)
            .unwrap();
        }

        let mut ticker = Ticker::every(Duration::from_millis(20));
        select(ticker.next(), display.flush()).await;
    }
}
