-- Add up migration script here

-- sqlfluff:dialect:sqlite

CREATE TABLE IF NOT EXISTS sensor_readings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sensor_id TEXT NOT NULL,
    topic TEXT NOT NULL,
    timestamp DATETIME NOT NULL,
    temperature REAL NOT NULL,
    pressure REAL NOT NULL,
    humidity REAL NOT NULL
);

CREATE INDEX idx_sensor_time ON sensor_readings (sensor_id, timestamp DESC);
