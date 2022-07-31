use std::sync::{Arc, RwLock};
use crate::{DataContainer, GuiSettingsContainer, Print};

pub fn serial_thread(bijou_settings: GuiSettingsContainer,
                     device_lock: Arc<RwLock<String>>,
                     raw_data_lock: Arc<RwLock<DataContainer>>,
                     print_lock: Arc<RwLock<Vec<Print>>>,
                     connected_lock: Arc<RwLock<bool>>) {

}