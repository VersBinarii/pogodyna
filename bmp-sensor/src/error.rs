#[derive(Debug, defmt::Format)]
pub enum SensorError {
    Measurement,
    Network,
    Protocol,
    Timeout,
    Disconnected,
}
