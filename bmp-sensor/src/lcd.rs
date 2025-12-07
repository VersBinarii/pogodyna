use core::cell::RefCell;
use defmt::error;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice;
use embassy_sync::blocking_mutex::{raw::NoopRawMutex, Mutex};
use embassy_time::Delay;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::{pixelcolor::Rgb565, prelude::RgbColor};
use esp_hal::peripherals::{GPIO10, GPIO6, GPIO7, GPIO8, GPIO9, SPI2};
use esp_hal::spi::master::Config as SpiConfig;
use esp_hal::{
    gpio::{Level, Output, OutputConfig},
    spi::{master::Spi, Mode},
    time::Rate,
    Blocking,
};
use st7735_lcd::{Orientation, ST7735};
use static_cell::StaticCell;

type Display = ST7735<
    SpiDevice<'static, NoopRawMutex, Spi<'static, Blocking>, Output<'static>>,
    Output<'static>,
    Output<'static>,
>;

static SPI_BUS: StaticCell<Mutex<NoopRawMutex, RefCell<Spi<'_, Blocking>>>> = StaticCell::new();

pub fn initialize_display(
    spi: SPI2<'static>,
    mosi: GPIO6<'static>,
    sclk: GPIO7<'static>,
    cs: GPIO8<'static>,
    dc: GPIO9<'static>,
    rst: GPIO10<'static>,
) -> Result<Display, ()> {
    let cs = Output::new(cs, Level::High, OutputConfig::default());
    let dc = Output::new(dc, Level::Low, OutputConfig::default());
    let rst = Output::new(rst, Level::Low, OutputConfig::default());
    let spi = Spi::new(
        spi,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(1))
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sclk)
    .with_mosi(mosi);

    let spi_bus = SPI_BUS.init(Mutex::new(RefCell::new(spi)));
    let spi_device = SpiDevice::new(spi_bus, cs);
    let mut display = st7735_lcd::ST7735::new(spi_device, dc, rst, true, false, 128, 160);
    if let Err(e) = display.init(&mut Delay) {
        error!("Failed to initialize display: {:?}", e);
    }
    if let Err(e) = display.clear(Rgb565::GREEN) {
        error!("Failed to clear screen: {:?}", e);
    }
    display.set_orientation(&Orientation::Portrait).unwrap();
    Ok(display)
}

// let style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLUE);
//
// // Create a text at position (20, 30) and draw it using the previously defined style
// Text::new("Hello Rust!", Point::new(20, 30), style)
//     .draw(&mut display)
//     .unwrap();
