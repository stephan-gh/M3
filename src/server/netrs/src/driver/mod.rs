///Conditional include of the driver
#[cfg(target_os = "linux")]
#[path = "host/mod.rs"]
pub mod driver;

#[cfg(target_os = "none")]
#[path = "gem5/mod.rs"]
pub mod driver;
