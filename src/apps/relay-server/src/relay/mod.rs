//! Core relay logic: room management and message routing.

pub mod device_manager;
pub mod room;

pub use device_manager::DeviceManager;
pub use room::RoomManager;
