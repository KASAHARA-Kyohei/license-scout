use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

pub fn with_spinner<T, F>(message: &str, action: F) -> Result<T>
where
    F: FnOnce(&ProgressBar) -> Result<T>,
{
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_chars("/|\\- "),
    );
    spinner.set_message(message.to_string());

    let result = action(&spinner);
    match &result {
        Ok(_) => spinner.finish_with_message(format!("{message} 完了")),
        Err(_) => spinner.finish_and_clear(),
    }
    result
}
