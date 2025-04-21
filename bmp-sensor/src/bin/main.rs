#![no_std]
#![no_main]

use bme280::i2c::AsyncBME280;
use bmp_sensor::mqtt::{MqttClientState, MqttConnector};
use core::net::Ipv4Addr;
use defmt::{error, info};
use embassy_net::{tcp::TcpSocket, Runner, StackResources};
use esp_alloc as _;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
    rng::Rng,
};
use heapless::String;

use embassy_executor::Spawner;
use embassy_time::{Delay, Duration, Timer};
use esp_wifi::wifi::{
    ClientConfiguration, Configuration, WifiController, WifiDevice, WifiError, WifiEvent, WifiState,
};
use esp_wifi::EspWifiController;

use bmp_sensor::mqtt::mqtt_client::MqttClient;

const SSID: &str = env!("SSID");
const WIFI_KEY: &str = env!("WIFI_KEY");
const BASE_STATION_ADDRESS: &str = env!("BASE_STATION_ADDRESS");
const BASE_STATION_PORT: &str = env!("BASE_STATION_PORT");

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
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

    let mut rng = Rng::new(peripherals.RNG);
    let timer1 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl = mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng, peripherals.RADIO_CLK).unwrap()
    );
    let (controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );
    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(runner)).ok();

    info!("Initilizeing i2c");
    let i2c_driver = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .into_async()
        .with_sda(peripherals.GPIO5)
        .with_scl(peripherals.GPIO6);
    let mut bme280 = AsyncBME280::new_primary(i2c_driver);
    info!("Initializing BME");
    match bme280.init(&mut Delay).await {
        Ok(_) => info!("Init ok"),
        Err(e) => error!("Error: {:?}", e),
    }

    loop {
        info!("checking link state");
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(embassy_time::Duration::from_secs(5)));
    let remote_endpoint = (
        BASE_STATION_ADDRESS.parse::<Ipv4Addr>().unwrap(),
        BASE_STATION_PORT.parse().unwrap(),
    );
    let mut _mqtt = MqttClient::new(socket, remote_endpoint, "outside-sensor");
    info!("connecting...");
    let _ = _mqtt.connect().await.unwrap();
    let mut mqtt = MqttConnector {
        state: MqttClientState::Connected,
        client: _mqtt,
    };
    info!("connected!");
    loop {
        use core::fmt::Write;
        if !mqtt.is_connected() {
            loop {
                if mqtt.reconnect().await.is_ok() {
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

        info!("trying to publish update");
        if let Err(_) = mqtt
            .publish("sensor/temperature", buf.trim().as_bytes())
            .await
        {
            error!("Error while publishing update. Will try to reconnect...");
        }

        Timer::after(Duration::from_millis(3000)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            info!("We're connected - waiting for disconnection");
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: WIFI_KEY.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            info!("Starting wifi");
            controller.start_async().await.unwrap();
            info!("Wifi started!");
        }
        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => info!("Wifi connected!"),
            Err(e) => {
                info!("Failed to connect to wifi");
                match e {
                    WifiError::Unsupported => error!("Unsupported mode"),
                    WifiError::Disconnected => error!("Disconneced"),
                    WifiError::NotInitialized => error!("Not Initialized"),
                    WifiError::UnknownWifiMode => error!("Unknown WIFI mode"),
                    WifiError::InternalError(_) => error!("Internal error"),
                }
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}
