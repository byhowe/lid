use std::ffi::OsStr;
use std::fmt::Display;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType
{
    /// Battery device.
    Battery,
    /// AC adapter (i.e., the power supply) of a device.
    Adapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status
{
    Discharging,
    Charging,
    Unknown,
}

impl Status
{
    fn read_from_battery_device(dev: &udev::Device) -> Self
    {
        let status = dev
            .property_value("POWER_SUPPLY_STATUS")
            .map(OsStr::to_str)
            .unwrap_or_default()
            .unwrap_or_default();
        match status {
            "Charging" => Self::Charging,
            "Discharging" | "Not charging" => Self::Discharging,
            _ => Self::Unknown,
        }
    }

    fn read_from_adapter_device(dev: &udev::Device) -> Self
    {
        let online = dev
            .property_value("POWER_SUPPLY_ONLINE")
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or(-1);
        match online {
            0 => Status::Discharging,
            1 | 2 => Status::Charging,
            _ => Status::Unknown,
        }
    }
}

impl Display for Status
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match *self {
            Status::Discharging => write!(f, "Discharging"),
            Status::Charging => write!(f, "Charging"),
            Status::Unknown => write!(f, "Unknown"),
        }
    }
}

pub struct PowerSupply
{
    bat: udev::Device,
    adp: udev::Device,

    status: Status,
    status_changed: bool,
}

impl PowerSupply
{
    pub fn new() -> io::Result<Self>
    {
        let mut enumerator = udev::Enumerator::new()?;
        enumerator.match_subsystem("power_supply")?;
        let devices = enumerator
            .scan_devices()?
            .filter_map(|dev| Self::device_type(&dev).map(|t| (t, dev)))
            .collect::<Vec<_>>();

        assert!(
            devices.len() == 2,
            "Failed to find two power supply devices!"
        );

        Ok(Self {
            bat: devices
                .iter()
                .find(|(dev_type, _)| *dev_type == DeviceType::Battery)
                .unwrap()
                .1
                .clone(),
            adp: devices
                .iter()
                .find(|(dev_type, _)| *dev_type == DeviceType::Adapter)
                .unwrap()
                .1
                .clone(),
            status: Status::Unknown,
            status_changed: true,
        })
    }

    pub fn set_device(&mut self, dev: udev::Device)
    {
        match Self::device_type(&dev) {
            Some(DeviceType::Battery) => self.bat = dev,
            Some(DeviceType::Adapter) => self.adp = dev,
            None => todo!(),
        }
    }

    #[must_use]
    fn device_type(dev: &udev::Device) -> Option<DeviceType>
    {
        match dev.parent()?.driver()?.to_str()? {
            "battery" => Some(DeviceType::Battery),
            "ac" => Some(DeviceType::Adapter),
            _ => {
                let sysname = dev.sysname().to_string_lossy();
                if sysname.starts_with("BAT") {
                    Some(DeviceType::Battery)
                } else if sysname.starts_with("ADP") {
                    Some(DeviceType::Adapter)
                } else {
                    None
                }
            }
        }
    }

    #[must_use]
    pub fn charging_status_changed(&self) -> bool
    {
        self.status_changed
    }

    /// Fetch the current status and set the last charging status to the current
    /// one.
    pub fn set_charging_status(&mut self)
    {
        let status = match Status::read_from_adapter_device(&self.adp) {
            Status::Unknown => Status::read_from_battery_device(&self.bat),
            status => status,
        };
        self.status_changed = status != self.status;
        self.status = status;
    }

    /// Returns the last charging status detected by the udev driver.
    #[must_use]
    pub fn charging_status(&self) -> Status
    {
        self.status
    }
}
