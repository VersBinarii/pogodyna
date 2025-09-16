use defmt::{debug, error, info};
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{
    peripherals::{RNG, TIMG0, WIFI},
    rng::Rng,
    timer::timg::TimerGroup,
};
use esp_wifi::{
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiError, WifiEvent,
        WifiState,
    },
    EspWifiController,
};

const SSID: &str = env!("SSID");
const WIFI_KEY: &str = env!("WIFI_KEY");

macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

pub async fn setup_wifi<'a>(
    wifi: WIFI<'static>,
    rng: RNG<'_>,
    timg0: TIMG0<'static>,
) -> Result<(WifiController<'a>, Stack<'a>, Runner<'a, WifiDevice<'a>>), ()> {
    let mut rng = Rng::new(rng);
    let timer1 = TimerGroup::new(timg0);
    let esp_wifi_ctrl = mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer1.timer0, rng).unwrap()
    );
    let (controller, interfaces) = esp_wifi::wifi::new(esp_wifi_ctrl, wifi).unwrap();
    let config = embassy_net::Config::dhcpv4(Default::default());
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner) = embassy_net::new(
        interfaces.sta,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        seed,
    );
    Ok((controller, stack, runner))
}

pub async fn wifi_connection(mut controller: WifiController<'_>) {
    loop {
        if esp_wifi::wifi::wifi_state() == WifiState::StaConnected {
            debug!("We're connected - waiting for disconnection");
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.into(),
                password: WIFI_KEY.into(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            controller.start_async().await.unwrap();
            debug!("Wifi started!");
        }
        info!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => debug!("Wifi connected!"),
            Err(e) => {
                info!("Failed to connect to wifi");
                match e {
                    WifiError::Unsupported => error!("Unsupported mode"),
                    WifiError::Disconnected => error!("Disconneced"),
                    WifiError::NotInitialized => error!("Not Initialized"),
                    WifiError::UnknownWifiMode => error!("Unknown WIFI mode"),
                    WifiError::InternalError(_) => error!("Internal error"),
                    _ => error!("Unknown error"),
                }
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}
