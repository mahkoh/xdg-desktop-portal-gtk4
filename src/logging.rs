use {
    log::{Level, LevelFilter},
    std::{
        env,
        fs::File,
        mem::ManuallyDrop,
        os::{fd::FromRawFd, linux::fs::MetadataExt},
    },
};

pub fn init() {
    let mut builder = env_logger::builder();
    if stderr_is_journal() {
        builder.format(|f, r| {
            use std::io::Write;
            let level = match r.level() {
                Level::Error => 3,
                Level::Warn => 4,
                Level::Info => 6,
                Level::Debug => 7,
                Level::Trace => 7,
            };
            write!(f, "<{level}>")?;
            if let Some(path) = r.module_path() {
                write!(f, "{path}: ")?;
            }
            writeln!(f, "{}", r.args())
        });
    } else {
        builder.default_format();
    }
    builder
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();
}

fn stderr_is_journal() -> bool {
    let Ok(journal_stream) = env::var("JOURNAL_STREAM") else {
        return false;
    };
    let Some((dev, ino)) = journal_stream.split_once(':') else {
        return false;
    };
    let Ok(dev) = dev.parse::<u64>() else {
        return false;
    };
    let Ok(ino) = ino.parse::<u64>() else {
        return false;
    };
    let stderr = unsafe { ManuallyDrop::new(File::from_raw_fd(2)) };
    let Ok(metadata) = stderr.metadata() else {
        return false;
    };
    metadata.st_dev() == dev && metadata.st_ino() == ino
}
