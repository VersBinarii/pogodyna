#![no_std]
#![no_main]

use bme280::i2c::AsyncBME280;
use bmp_sensor::mqtt::MqttConnector;
use bmp_sensor::wifi::{setup_wifi, wifi_connection};
use core::net::Ipv4Addr;
use defmt::{error, info};
use embassy_net::{tcp::TcpSocket, Runner};
use esp_alloc as _;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
};
use heapless::String;

use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use esp_wifi::wifi::{WifiController, WifiDevice};

const BASE_STATION_ADDRESS: &str = env!("BASE_STATION_ADDRESS");
const BASE_STATION_PORT: &str = env!("BASE_STATION_PORT");

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    rtt_target::rtt_init_defmt!();

    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    info!("Embassy initialized!");

    let (controller, stack, runner) =
        setup_wifi(peripherals.WIFI, peripherals.RNG, peripherals.TIMG0)
            .await
            .unwrap();

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    loop {
        info!("checking link state");
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(1000)).await;
    }

    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));
    let remote_endpoint = (
        BASE_STATION_ADDRESS.parse::<Ipv4Addr>().unwrap(),
        BASE_STATION_PORT.parse().unwrap(),
    )
        .into();
    let mut mqtt = MqttConnector::new(socket, remote_endpoint, "outside_sensor");

    let i2c_driver = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .into_async()
        .with_sda(peripherals.GPIO4)
        .with_scl(peripherals.GPIO5);

    let mut bme280 = AsyncBME280::new_primary(i2c_driver);
    if let Err(e) = bme280.init(&mut Delay).await {
        error!("Error initializing BME: {:?}", e);
    }
    loop {
        use core::fmt::Write;
        if !mqtt.is_connected() {
            loop {
                if mqtt.connect().await.is_ok() {
                    break;
                } else {
                    error!("Failed to reconnect");
                }
                Timer::after(Duration::from_millis(3000)).await;
            }
        }
        let measurement = bme280.measure(&mut Delay).await.unwrap();
        let mut buf: String<64> = String::new();
        let _ = write!(
            buf,
            "{{\"t\":\"{}\",\"p\":\"{}\",\"h\":\"{}\"}}",
            measurement.temperature, measurement.pressure, measurement.humidity
        );

        if let Err(e) = mqtt.publish("sensor/update", buf.trim().as_bytes()).await {
            error!("Error while publishing update: {}", e);
        }

        Timer::after(Duration::from_millis(3000)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn connection(controller: WifiController<'static>) {
    wifi_connection(controller).await
}
