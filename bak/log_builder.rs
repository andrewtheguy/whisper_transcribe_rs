use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log::LevelFilter;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::config::{Appender, Config, Root};

pub struct MyLoggerBuilder {
    path: String,
    max_size: u64,
    file_count: u32,
}

impl MyLoggerBuilder {
    // Create a new builder with default values
    pub fn new() -> Self {
        MyLoggerBuilder {
            path: "logs/app.log".to_string(),
            max_size: 1 * 1024 * 1024, // 1MB
            file_count: 5, // Keep 5 files
        }
    }

    // Set the path dynamically
    pub fn path(mut self, path: &str) -> Self {
        self.path = path.to_string();
        self
    }

    // Set the maximum size of each log file
    pub fn max_size(mut self, max_size: u64) -> Self {
        self.max_size = max_size;
        self
    }

    // Set the maximum number of log files to keep
    pub fn file_count(mut self, file_count: u32) -> Self {
        self.file_count = file_count;
        self
    }

    // Build and initialize the logger with the specified settings
    pub fn build(self) {
        // Create the rolling file roller with the specified file count
        let roller = FixedWindowRoller::builder()
            .base(1)
            .build(&format!("{}.{{}}", self.path), self.file_count)
            .unwrap();

        // Create the size trigger with the specified max size
        let trigger = SizeTrigger::new(self.max_size);

        // Combine the roller and trigger into a compound policy
        let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

        // Create the rolling file appender
        let file_appender = RollingFileAppender::builder()
            .build(self.path, Box::new(policy))
            .unwrap();

        // Build the log4rs config
        let config = Config::builder()
            .appender(Appender::builder().build("file", Box::new(file_appender)))
            .build(Root::builder().appender("file").build(LevelFilter::Info))
            .unwrap();

        // Initialize the logger
        log4rs::init_config(config).unwrap();
    }
}