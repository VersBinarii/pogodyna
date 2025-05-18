# Installation

## Base station 

```bash
cargo install --git https://github.com/VersBinarii/pogodyna.git
```

## MQTT Broker

Eventually we will want this system to work with any 
MQTT broker but for now its tested only with [Mosquittto](https://github.com/eclipse-mosquitto/mosquitto)

```bash
sudo pacman -S mosquitto
```

## Sensor
For now the sensor needs to be build and flashed with the 
[bmp-sensor](../../../../bmp-sensor/) firmware.

## Configuration
Following is the example configuration required for building the base-station and the sensor:
```bash
SSID=my_home_wifi
WIFI_KEY=my_home_wifi_password
BASE_STATION_ADDRESS=192.168.1.200
BASE_STATION_PORT=1883
DATABASE_URL="sqlite://sensor_readings.db"
LOG_DIRECTORY="./logs/basestation"
RUST_LOG=debug,sqlx=info
```
