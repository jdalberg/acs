use std::{collections::HashMap, env};

use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;

const DATA_MODEL_FILE: &str = "device_data.json";

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DeviceInfo {
    pub serial_number: String,
    custom_info: String,
    pub software_version: String,
    pub hardware_version: String,
    pub oui: String,
    pub manufacturer: String,
    pub product_class: String,
    pub acs_url: String,
    pub connection_request_url: String,
    pub parameter_key: String,
    #[serde(rename = "Other")]
    other_params: HashMap<String, String>,
    #[serde(skip)]
    session_cookie: Option<String>,
}

impl DeviceInfo {
    pub fn new() -> Self {
        let device_info = if let Ok(data) = fs::read_to_string(DATA_MODEL_FILE) {
            if let Ok(mut device_info) = serde_json::from_str::<DeviceInfo>(&data) {
                device_info.session_cookie = None;
                device_info
            } else {
                Self::create_default()
            }
        } else {
            Self::create_default()
        };
        device_info.save();
        device_info
    }

    pub fn start_session(&mut self, id: String) {
        debug!("Starting session with ID: {}", id);
        self.session_cookie = Some(id);
    }

    pub fn get_session_cookie(&self) -> Option<String> {
        self.session_cookie.as_ref().map(|cookie| cookie.clone())
    }
    pub fn end_session(&mut self) {
        self.session_cookie = None;
    }

    pub fn create_default() -> Self {
        let acs_url =
            env::var("ACS_URL").unwrap_or_else(|_| "http://127.0.0.1:7548/cwmp".to_string());
        let connection_request_url = env::var("CPE_CONNECTION_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:7547/connection".to_string());
        let parameter_key =
            env::var("CPE_PARAMETER_KEY").unwrap_or_else(|_| "default_key".to_string());

        let mut other_params = HashMap::new();
        other_params.insert(
            "Device.Other.Param1".to_string(),
            "default_value1".to_string(),
        );

        Self {
            serial_number: "123456789".to_string(),
            custom_info: "DefaultCustomInfo".to_string(),
            software_version: "1.0.0".to_string(),
            hardware_version: "1.0".to_string(),
            oui: "000000".to_string(),
            manufacturer: "ExampleCorp".to_string(),
            product_class: "ExampleDevice".to_string(),
            acs_url,
            connection_request_url,
            parameter_key,
            other_params,
            session_cookie: None,
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(DATA_MODEL_FILE, json);
        }
    }

    pub fn get_parameter_value(&self, path: &str) -> Option<String> {
        // Remove "Device." prefix if it exists
        let path = path.strip_prefix("Device.").unwrap_or(path);

        match path {
            // Direct mappings
            "DeviceInfo.SerialNumber" => Some(self.serial_number.clone()),
            "DeviceInfo.CustomInfo" => Some(self.custom_info.clone()),
            "DeviceInfo.SoftwareVersion" => Some(self.software_version.clone()),
            "DeviceInfo.HardwareVersion" => Some(self.hardware_version.clone()),
            "DeviceInfo.ManufacturerOUI" => Some(self.oui.clone()),
            "DeviceInfo.Manufacturer" => Some(self.manufacturer.clone()),
            "DeviceInfo.ProductClass" => Some(self.product_class.clone()),
            "ManagementServer.URL" => Some(self.acs_url.clone()),
            "ManagementServer.ConnectionRequestURL" => Some(self.connection_request_url.clone()),
            "ManagementServer.ParameterKey" => Some(self.parameter_key.clone()),

            // Handle Other parameters
            path if path.starts_with("Other.") => {
                let param_name = path.strip_prefix("Other.").unwrap_or(path);
                self.other_params.get(param_name).cloned()
            }
            _ => None,
        }
    }

    pub fn set_parameter_value(&mut self, path: &str, value: &str) -> Result<(), u32> {
        // Remove "Device." prefix if it exists
        let path = path.strip_prefix("Device.").unwrap_or(path);

        if !self.is_parameter_writable(path) {
            return Err(if self.get_parameter_value(path).is_some() {
                9001 // Parameter exists but is read-only
            } else {
                9005 // Parameter doesn't exist
            });
        }

        match path {
            "DeviceInfo.CustomInfo" => self.custom_info = value.to_string(),
            "ManagementServer.URL" => self.acs_url = value.to_string(),
            "ManagementServer.ConnectionRequestURL" => {
                self.connection_request_url = value.to_string()
            }
            "ManagementServer.ParameterKey" => self.parameter_key = value.to_string(),

            // Handle Other parameters
            path if path.starts_with("Other.") => {
                let param_name = path.strip_prefix("Other.").unwrap_or(path);
                self.other_params
                    .insert(param_name.to_string(), value.to_string());
            }
            _ => return Err(9005), // Invalid Parameter Name
        }

        self.save();
        Ok(())
    }

    pub fn is_parameter_writable(&self, path: &str) -> bool {
        // Remove "Device." prefix if it exists
        let path = path.strip_prefix("Device.").unwrap_or(path);

        match path {
            // Writable parameters
            "DeviceInfo.CustomInfo" => true,
            "ManagementServer.URL" => true,
            "ManagementServer.ConnectionRequestURL" => true,
            "ManagementServer.ParameterKey" => true,

            // Read-only parameters
            "DeviceInfo.SerialNumber" => false,
            "DeviceInfo.SoftwareVersion" => false,
            "DeviceInfo.HardwareVersion" => false,
            "DeviceInfo.ManufacturerOUI" => false,
            "DeviceInfo.Manufacturer" => false,
            "DeviceInfo.ProductClass" => false,

            // Other parameters are always writable
            path if path.starts_with("Other.") => true,

            _ => false,
        }
    }
}
