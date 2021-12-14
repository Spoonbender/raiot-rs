use std::fmt;

/// A device identity
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    /// The Device ID
    pub device_id: String,
}

impl From<String> for DeviceIdentity {
    fn from(device_id: String) -> Self {
        DeviceIdentity {
            device_id: device_id,
        }
    }
}

impl fmt::Display for DeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.device_id)
    }
}

/// A device module identity
#[derive(Clone, Debug)]
pub struct ModuleIdentity {
    /// The device ID
    pub device_id: String,

    /// The module ID
    pub module_id: String,
}

impl fmt::Display for ModuleIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.device_id, self.module_id)
    }
}

/// A client identity (device or module)
#[derive(Clone, Debug)]
pub enum ClientIdentity {
    /// A device identity
    Device(DeviceIdentity),

    /// A module identity
    Module(ModuleIdentity),
}

impl ClientIdentity {
    /// Creates a Device Identity from the specified device_id
    pub fn from_device_id(device_id: &str) -> ClientIdentity {
        ClientIdentity::Device(DeviceIdentity {
            device_id: device_id.to_owned(),
        })
    }

    /// Creates a Module Identity from the specified device_id and module_id
    pub fn from_module_id(device_id: &str, module_id: &str) -> ClientIdentity {
        ClientIdentity::Module(ModuleIdentity {
            device_id: device_id.to_owned(),
            module_id: module_id.to_owned(),
        })
    }
}

impl fmt::Display for ClientIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientIdentity::Device(device_id) => write!(f, "{}", device_id),
            ClientIdentity::Module(module_id) => write!(f, "{}", module_id),
        }
    }
}
