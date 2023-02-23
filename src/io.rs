use crate::DataContainer;
use csv::WriterBuilder;
use std::error::Error;
use std::path::PathBuf;

pub fn save_to_csv(data: &DataContainer, file_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(file_path)?;
    // serialize does not work, so we do it with a loop..
    let mut header = vec!["time".to_string()];
    for (i, _value) in data.dataset.iter().enumerate() {
        header.push(format!("value {i}"));
    }
    wtr.write_record(header)?;
    for j in 0..data.dataset[0].len() {
        let mut data_to_write = vec![data.time[j].to_string()];
        for value in data.dataset.iter() {
            data_to_write.push(value[j].to_string());
        }
        wtr.write_record(&data_to_write)?;
    }
    wtr.flush()?;
    Ok(())
}
