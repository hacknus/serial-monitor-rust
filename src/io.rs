use std::error::Error;

use csv::WriterBuilder;

use crate::{CsvOptions, DataContainer};

pub fn save_to_csv(data: &DataContainer, csv_options: &CsvOptions) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new()
        .has_headers(false)
        .from_path(&csv_options.file_path)?;
    // serialize does not work, so we do it with a loop..
    let mut header = vec!["Time [ms]".to_string()];
    header.extend_from_slice(&data.names);
    wtr.write_record(header)?;
    for j in 0..data.dataset[0].len() {
        let time = if csv_options.save_absolute_time {
            data.absolute_time[j].to_string()
        } else {
            data.time[j].to_string()
        };
        let mut data_to_write = vec![time];
        for value in data.dataset.iter() {
            data_to_write.push(value[j].to_string());
        }
        wtr.write_record(&data_to_write)?;
    }
    wtr.flush()?;
    Ok(())
}
