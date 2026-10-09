use std::fmt;

use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConnectorName(String);

impl ConnectorName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectorName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable key an assignment is stored under: the EDID identity when the panel reports one,
/// otherwise the connector name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MonitorKey(String);

impl MonitorKey {
    #[must_use]
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MonitorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorIdentity {
    pub connector: ConnectorName,
    pub vendor: String,
    pub product: String,
    pub serial: String,
}

impl MonitorIdentity {
    #[must_use]
    pub fn key(&self) -> MonitorKey {
        if self.vendor.is_empty() && self.product.is_empty() && self.serial.is_empty() {
            return MonitorKey(format!("connector:{}", self.connector));
        }

        MonitorKey(format!(
            "edid:{}/{}/{}",
            self.vendor, self.product, self.serial
        ))
    }

    #[must_use]
    pub fn connector_key(&self) -> MonitorKey {
        MonitorKey(format!("connector:{}", self.connector))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// A rectangle in logical (scaled) pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    #[must_use]
    pub fn size(self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    #[must_use]
    pub fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    #[must_use]
    pub fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        let width = u32::try_from(right - i64::from(x)).unwrap_or(u32::MAX);
        let height = u32::try_from(bottom - i64::from(y)).unwrap_or(u32::MAX);

        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub identity: MonitorIdentity,
    pub logical: Rectangle,
    pub scale: f64,
    pub is_primary: bool,
}
