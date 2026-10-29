//! `LibreHardwareMonitor`: the temperature and the power of the processor and the fans, which need its `PawnIO`
//! driver. The panel installs nothing: it asks the web server of a running `LibreHardwareMonitor` on this PC
//! (Options → Remote Web Server → Run) for its tree, and without an answer the screen says how to turn it on.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use serde::Deserialize;
use vitals_ipc::FanReading;

/// The port `LibreHardwareMonitor` serves on until the user changes it
pub const PORT: u16 = 8085;
/// Windows retries a connection to a closed port of this PC for about two seconds rather than failing at once; a
/// server that listens here accepts in microseconds, so a missing one costs this per reading
const CONNECT_DEADLINE: Duration = Duration::from_millis(200);
/// A server that hangs costs at most this per read
const READ_DEADLINE: Duration = Duration::from_secs(1);
/// The tree of a PC with many sensors is a few hundred kilobytes
const MAX_ANSWER: u64 = 8 * 1024 * 1024;

/// The temperature of the processor in the order it is preferred: AMD gives Tctl/Tdie as one sensor, or both when
/// Tctl carries an offset; Intel gives the package.
const CPU_TEMPERATURES: [&str; 4] = [
    "Core (Tctl/Tdie)",
    "Core (Tdie)",
    "Core (Tctl)",
    "CPU Package",
];
const CPU_POWERS: [&str; 2] = ["Package", "CPU Package"];
/// Intel's distances to the throttle point are counted down from it, not temperatures
const NOT_A_TEMPERATURE: &str = "Distance to TjMax";

/// What the panel takes from the tree of `LibreHardwareMonitor`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LhmReading {
    pub cpu_temp_c: Option<f64>,
    pub cpu_power_w: Option<f64>,
    pub fans: Vec<FanReading>,
}

/// A node of `data.json`: the computer, a piece of hardware, a group of a type or a sensor; only a sensor has an id
/// and a type. The raw values are in degrees Celsius whatever unit the user picked, formatted in the language of
/// Windows: `61,4 °C` in Russian.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    #[serde(default)]
    text: String,
    sensor_id: Option<String>,
    #[serde(rename = "Type")]
    kind: Option<String>,
    raw_value: Option<String>,
    raw_max: Option<String>,
    #[serde(default)]
    children: Vec<Node>,
}

#[derive(Debug)]
struct Sensor<'a> {
    id: &'a str,
    kind: &'a str,
    name: &'a str,
    value: Option<f64>,
    max: Option<f64>,
}

/// The number a formatted value starts with: `61,4 °C`, `1254 RPM`; `NaN`, `-` and an empty value are none.
fn number_of(text: &str) -> Option<f64> {
    let number: String = text
        .trim()
        .chars()
        .take_while(|char| char.is_ascii_digit() || matches!(char, '.' | ',' | '-'))
        .collect();
    let value: f64 = number.replace(',', ".").parse().ok()?;
    value.is_finite().then_some(value)
}

fn collect_sensors<'a>(node: &'a Node, sensors: &mut Vec<Sensor<'a>>) {
    if let (Some(id), Some(kind)) = (&node.sensor_id, &node.kind) {
        sensors.push(Sensor {
            id,
            kind,
            name: &node.text,
            value: node.raw_value.as_deref().and_then(number_of),
            max: node.raw_max.as_deref().and_then(number_of),
        });
    }
    for child in &node.children {
        collect_sensors(child, sensors);
    }
}

fn is_cpu(sensor: &Sensor<'_>) -> bool {
    sensor.id.starts_with("/amdcpu/") || sensor.id.starts_with("/intelcpu/")
}

/// The first of `names` the processor reports, else the highest value of its kind: a sensor the user renamed.
fn cpu_value_of(sensors: &[Sensor<'_>], kind: &str, names: &[&str]) -> Option<f64> {
    let own: Vec<&Sensor<'_>> = sensors
        .iter()
        .filter(|sensor| is_cpu(sensor) && sensor.kind == kind && sensor.value.is_some())
        .collect();
    names
        .iter()
        .find_map(|name| own.iter().find(|sensor| sensor.name == *name))
        .and_then(|sensor| sensor.value)
        .or_else(|| {
            own.iter()
                .filter(|sensor| !sensor.name.contains(NOT_A_TEMPERATURE))
                .filter_map(|sensor| sensor.value)
                .reduce(f64::max)
        })
}

/// What the panel takes from `data.json`: the processor's temperature and power, and the fans of the board and the
/// coolers that spin or spun since `LibreHardwareMonitor` started. A header with nothing on it never spins; a card's
/// fans come from `nvidia-smi`.
pub fn reading_of(json: &str) -> Option<LhmReading> {
    let root: Node = serde_json::from_str(json).ok()?;
    let mut sensors = Vec::new();
    collect_sensors(&root, &mut sensors);
    Some(LhmReading {
        cpu_temp_c: cpu_value_of(&sensors, "Temperature", &CPU_TEMPERATURES),
        cpu_power_w: cpu_value_of(&sensors, "Power", &CPU_POWERS),
        fans: sensors
            .iter()
            .filter(|sensor| sensor.kind == "Fan" && !sensor.id.starts_with("/gpu"))
            .filter_map(|sensor| {
                let rpm = sensor.value?;
                (rpm > 0.0 || sensor.max.is_some_and(|max| max > 0.0)).then(|| FanReading {
                    name: sensor.name.to_owned(),
                    rpm,
                })
            })
            .collect(),
    })
}

/// The body of `GET /data.json` from `address` on this PC, when a server answers 200 in time.
pub fn fetch_tree(address: SocketAddr) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_DEADLINE).ok()?;
    stream.set_read_timeout(Some(READ_DEADLINE)).ok()?;
    stream.set_write_timeout(Some(READ_DEADLINE)).ok()?;
    // One write: a request in pieces could meet a server that answers after the first and closes on the rest
    let request =
        format!("GET /data.json HTTP/1.0\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).ok()?;
    let mut answer = Vec::new();
    stream.take(MAX_ANSWER).read_to_end(&mut answer).ok()?;
    let answer = String::from_utf8(answer).ok()?;
    let (head, body) = answer.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    (status == "200").then(|| body.to_owned())
}

/// What `LibreHardwareMonitor` on this PC reports now; none when it does not run or its web server is off.
pub fn read_local() -> Option<LhmReading> {
    fetch_tree(SocketAddr::from((Ipv4Addr::LOCALHOST, PORT)))
        .as_deref()
        .and_then(reading_of)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    /// The shape `LibreHardwareMonitor` 0.9.6 serves, cut to the nodes that matter, on a Russian Windows
    const TREE: &str = r#"{"id":0,"Text":"Sensor","Min":"Min","Value":"Value","Max":"Max","ImageURL":"","Children":[
      {"id":1,"Text":"PC","Min":"","Value":"","Max":"","ImageURL":"images_icon/computer.png","Children":[
        {"id":2,"Text":"ASRock X870 Pro RS","HardwareId":"/motherboard","Children":[
          {"id":3,"Text":"Nuvoton NCT6799D","HardwareId":"/lpc/nct6799d/0","Children":[
            {"id":4,"Text":"Temperatures","Children":[
              {"id":5,"Text":"Motherboard","SensorId":"/lpc/nct6799d/0/temperature/0","Type":"Temperature","Value":"38,0 °C","RawValue":"38,0 °C","RawMax":"39,0 °C","Children":[]}]},
            {"id":6,"Text":"Fans","Children":[
              {"id":7,"Text":"CPU Fan #1","SensorId":"/lpc/nct6799d/0/fan/1","Type":"Fan","Value":"1254 RPM","RawValue":"1254 RPM","RawMax":"1302 RPM","Children":[]},
              {"id":8,"Text":"Chassis Fan #1","SensorId":"/lpc/nct6799d/0/fan/2","Type":"Fan","Value":"0 RPM","RawValue":"0 RPM","RawMax":"812 RPM","Children":[]},
              {"id":9,"Text":"Chassis Fan #2","SensorId":"/lpc/nct6799d/0/fan/3","Type":"Fan","Value":"0 RPM","RawValue":"0 RPM","RawMax":"0 RPM","Children":[]}]}]}]},
        {"id":10,"Text":"AMD Ryzen 9 9900X","HardwareId":"/amdcpu/0","Children":[
          {"id":11,"Text":"Temperatures","Children":[
            {"id":12,"Text":"CCD1 (Tdie)","SensorId":"/amdcpu/0/temperature/3","Type":"Temperature","Value":"58,0 °C","RawValue":"58,0 °C","RawMax":"71,5 °C","Children":[]},
            {"id":13,"Text":"Core (Tctl/Tdie)","SensorId":"/amdcpu/0/temperature/2","Type":"Temperature","Value":"142,5 °F","RawValue":"61,4 °C","RawMax":"80,1 °C","Children":[]}]},
          {"id":14,"Text":"Powers","Children":[
            {"id":15,"Text":"Core #1 (SMU)","SensorId":"/amdcpu/0/power/1","Type":"Power","Value":"6,1 W","RawValue":"6,1 W","RawMax":"9,0 W","Children":[]},
            {"id":16,"Text":"Package","SensorId":"/amdcpu/0/power/0","Type":"Power","Value":"88,2 W","RawValue":"88,2 W","RawMax":"162,0 W","Children":[]}]}]},
        {"id":17,"Text":"NVIDIA GeForce RTX 4070 Ti SUPER","HardwareId":"/gpu-nvidia/0","Children":[
          {"id":18,"Text":"Fans","Children":[
            {"id":19,"Text":"GPU Fan 1","SensorId":"/gpu-nvidia/0/fan/1","Type":"Fan","Value":"0 RPM","RawValue":"0 RPM","RawMax":"1500 RPM","Children":[]}]}]}]}]}"#;

    #[test]
    fn reads_the_processor_and_the_fans_of_the_board() {
        let reading = reading_of(TREE).unwrap();
        // Tctl/Tdie over a CCD, in Celsius though the user shows Fahrenheit
        assert_eq!(reading.cpu_temp_c, Some(61.4));
        assert_eq!(reading.cpu_power_w, Some(88.2));
        // A stopped fan that spun stays, a header that never spun and the card's fan do not
        let fans: Vec<(&str, f64)> = reading
            .fans
            .iter()
            .map(|fan| (fan.name.as_str(), fan.rpm))
            .collect();
        assert_eq!(fans, [("CPU Fan #1", 1254.0), ("Chassis Fan #1", 0.0)]);
    }

    #[test]
    fn intel_gives_its_package() {
        let tree = r#"{"Text":"Sensor","Children":[{"Text":"Intel Core i7-13700K","HardwareId":"/intelcpu/0","Children":[
            {"Text":"Core Max","SensorId":"/intelcpu/0/temperature/17","Type":"Temperature","RawValue":"74.0 °C"},
            {"Text":"Core #1 Distance to TjMax","SensorId":"/intelcpu/0/temperature/18","Type":"Temperature","RawValue":"40.0 °C"},
            {"Text":"CPU Package","SensorId":"/intelcpu/0/temperature/16","Type":"Temperature","RawValue":"71.0 °C"},
            {"Text":"CPU Package","SensorId":"/intelcpu/0/power/0","Type":"Power","RawValue":"120.5 W"}]}]}"#;
        let reading = reading_of(tree).unwrap();
        assert_eq!(reading.cpu_temp_c, Some(71.0));
        assert_eq!(reading.cpu_power_w, Some(120.5));
        assert_eq!(reading.fans, Vec::<FanReading>::new());
    }

    #[test]
    fn a_renamed_sensor_falls_back_to_the_hottest() {
        let tree = r#"{"Text":"Sensor","Children":[
            {"Text":"CPU die","SensorId":"/amdcpu/0/temperature/2","Type":"Temperature","RawValue":"63,0 °C"},
            {"Text":"Core #1 Distance to TjMax","SensorId":"/intelcpu/0/temperature/9","Type":"Temperature","RawValue":"80,0 °C"},
            {"Text":"CCD1 (Tdie)","SensorId":"/amdcpu/0/temperature/3","Type":"Temperature","RawValue":"NaN"},
            {"Text":"Motherboard","SensorId":"/lpc/nct6799d/0/temperature/0","Type":"Temperature","RawValue":"90,0 °C"}]}"#;
        let reading = reading_of(tree).unwrap();
        assert_eq!(reading.cpu_temp_c, Some(63.0));
        assert_eq!(reading.cpu_power_w, None);
    }

    #[test]
    fn formatted_values_in_either_language() {
        assert_eq!(number_of("61,4 °C"), Some(61.4));
        assert_eq!(number_of("61.4 °C"), Some(61.4));
        assert_eq!(number_of("1254 RPM"), Some(1254.0));
        for text in ["", "-", "NaN", "не число", "°C"] {
            assert_eq!(number_of(text), None, "{text}");
        }
    }

    #[test]
    fn what_is_not_the_tree_reads_nothing() {
        assert_eq!(reading_of("<html>"), None);
        assert_eq!(
            reading_of(r#"{"Text":"Sensor"}"#),
            Some(LhmReading::default())
        );
    }

    /// A server that reads one request whole, answers it with `answer` and closes: closing on unread bytes would
    /// reset the connection rather than end it.
    fn serve(answer: &'static str) -> SocketAddr {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 512];
            while !request.ends_with(b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..read]);
            }
            assert!(request.starts_with(b"GET /data.json HTTP/1.0\r\n"));
            stream.write_all(answer.as_bytes()).unwrap();
        });
        address
    }

    #[test]
    fn fetches_the_tree_from_a_server_that_answers() {
        let address = serve(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\n\r\n{\"Text\":\"Sensor\"}",
        );
        assert_eq!(fetch_tree(address).as_deref(), Some(r#"{"Text":"Sensor"}"#));
        let address = serve("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        assert_eq!(fetch_tree(address), None);
    }

    #[test]
    fn nothing_listening_is_no_tree() {
        // A port bound and released: nothing listens on it now
        let address = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap();
        assert_eq!(fetch_tree(address), None);
    }
}
