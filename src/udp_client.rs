use core::str::FromStr;

use embassy_executor::Spawner;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    IpEndpoint, Stack,
};
use embassy_time::Timer;
use esp_backtrace as _;
use esp_wifi::wifi::{WifiDevice, WifiStaDevice};


use crate::bus::{NetSpeed, WiFiConnectStatus, NET_SPEED, WIFI_CONNECT_STATUS};

static SERVER_IP: &str = env!("SERVER_ADDRESS");
static LOCAL_PORT: u16 = 17891;

#[embassy_executor::task]
pub async fn receiving_net_speed(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    loop {
        let wifi_status_guard = WIFI_CONNECT_STATUS.lock().await;
        let wifi_status = *wifi_status_guard;
        drop(wifi_status_guard);

        match wifi_status {
            WiFiConnectStatus::Connected => break,
            _ => {}
        };

        Timer::after_millis(10).await;
    }

    static RX_BUF: static_cell::StaticCell<[u8; 4096]> = static_cell::StaticCell::new();
    let rx_buf = RX_BUF.init([0u8; 4096]);

    static TX_BUF: static_cell::StaticCell<[u8; 4096]> = static_cell::StaticCell::new();
    let tx_buf = TX_BUF.init([0u8; 4096]);

    static RX_META: static_cell::StaticCell<[PacketMetadata; 16]> = static_cell::StaticCell::new();
    let rx_meta = RX_META.init([PacketMetadata::EMPTY; 16]);

    static TX_META: static_cell::StaticCell<[PacketMetadata; 16]> = static_cell::StaticCell::new();
    let tx_meta = TX_META.init([PacketMetadata::EMPTY; 16]);

    let mut socket: UdpSocket<'static> = UdpSocket::new(stack, rx_meta, rx_buf, tx_meta, tx_buf);

    socket.bind(LOCAL_PORT).unwrap();

    static SOCKET: static_cell::StaticCell<UdpSocket<'static>> = static_cell::StaticCell::new();
    let socket = SOCKET.init(socket);

    let spawner = Spawner::for_current_executor().await;
    spawner.spawn(keep_alive(socket)).ok();
    spawner.spawn(receive_msg(socket)).ok();
}

#[embassy_executor::task]
async fn keep_alive(socket: &'static UdpSocket<'static>) {
    loop {
        let wifi_status_guard = WIFI_CONNECT_STATUS.lock().await;
        let wifi_status = *wifi_status_guard;
        drop(wifi_status_guard);

        match wifi_status {
            WiFiConnectStatus::Connected => {
                let msg: [u8; 2] = [0x01, 0x00];
                let ip_addr = IpEndpoint::from_str(SERVER_IP).unwrap();
                socket.send_to(&msg, ip_addr).await.unwrap();
                Timer::after_millis(5000).await;
            }
            _ => {
                Timer::after_millis(10).await;
            }
        };
    }
}

#[embassy_executor::task]
async fn receive_msg(socket: &'static UdpSocket<'static>) {
    let mut buf = [0u8; 48];

    loop {
        let (n, _) = socket.recv_from(&mut buf).await.unwrap();
        let mut speed = NetSpeed::default();
        if n >= 32 {
            speed.direct_up_bps = u64::from_le_bytes(buf[0..8].try_into().unwrap());
            speed.direct_down_bps = u64::from_le_bytes(buf[8..16].try_into().unwrap());
            speed.proxy_up_bps = u64::from_le_bytes(buf[16..24].try_into().unwrap());
            speed.proxy_down_bps = u64::from_le_bytes(buf[24..32].try_into().unwrap());
        }

        if n == 48 {
            speed.bypass_up_bps = u64::from_le_bytes(buf[32..40].try_into().unwrap());
            speed.bypass_down_bps = u64::from_le_bytes(buf[40..48].try_into().unwrap());
        }

        // println!("received {:?} bytes: {:?} ", n, speed);
        let mut net_speed_guard = NET_SPEED.lock().await;
        *net_speed_guard = speed;
        drop(net_speed_guard);
    }
}
