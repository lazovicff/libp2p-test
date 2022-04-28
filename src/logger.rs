use env_logger::Builder;
use log::LevelFilter;

pub fn init_logger() {
    let mut builder = Builder::from_default_env();

    builder
        .filter(None, LevelFilter::Info)
        .format_timestamp(None)
        .init();
}
