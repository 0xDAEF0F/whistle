use env_logger::{Builder, WriteStyle};

pub fn init(log_level: log::LevelFilter) {
    Builder::new()
        .filter_level(log_level)
        .format(|buf, record| {
            use std::io::Write;
            let timestamp = chrono::Local::now().format("%I:%M%p");
            let style = buf.default_level_style(record.level());
            let level_style = format!("{style}{}{style:#}", record.level());
            writeln!(
                buf,
                "[{} {} {}] {}",
                timestamp,
                level_style,
                record.target(),
                record.args()
            )
        })
        .format_level(true)
        .write_style(WriteStyle::Always)
        .init();
}
