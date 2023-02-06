use std::error::Error;
use std::path::PathBuf;
use csv::{WriterBuilder};
use crate::DataContainer;


pub fn save_to_csv(data: &DataContainer, file_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(file_path)?;
    // serialize does not work, so we do it with a loop..
    wtr.write_record(&["time", "t1", "t2", "t3", "t4", "t5", "pump_state",
        "pump", "heater_1_state", "heater_1", "heater_2_state", "heater_2"])?;
    for i in 0..data.time.len() {
        wtr.write_record(&[
            data.time[i].to_string(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}