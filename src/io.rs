use std::error::Error;
use std::path::PathBuf;

use csv::WriterBuilder;

use crate::DataContainer;

/// A set of options for saving data to a CSV file.
#[derive(Debug)]
pub struct FileOptions {
    pub file_path: PathBuf,
    pub save_absolute_time: bool,
    pub save_raw_traffic: bool,
}

pub fn save_to_csv(data: &DataContainer, csv_options: &FileOptions) -> Result<(), Box<dyn Error>> {
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
    if csv_options.save_raw_traffic {
        let mut path = csv_options.file_path.clone();
        let mut file_name = path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
            .replace(".csv", "");
        file_name += "raw.csv";
        path.set_file_name(file_name);
        save_raw(data, &path)?
    }
    Ok(())
}

pub fn save_raw(data: &DataContainer, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut wtr = WriterBuilder::new().has_headers(false).from_path(path)?;
    let header = vec![
        "Time [ms]".to_string(),
        "Abs Time [ms]".to_string(),
        "Raw Traffic".to_string(),
    ];
    wtr.write_record(header)?;

    for j in 0..data.dataset[0].len() {
        let mut data_to_write = vec![data.time[j].to_string(), data.absolute_time[j].to_string()];
        data_to_write.push(data.raw_traffic[j].payload.clone());
        wtr.write_record(&data_to_write)?;
    }
    wtr.flush()?;
    Ok(())
}
