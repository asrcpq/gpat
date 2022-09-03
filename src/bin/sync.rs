use gpat::{sync_git_to_gpat, sync_gpat_to_git};

struct Logger;

impl log::Log for Logger {
	fn enabled(&self, _: &log::Metadata<'_>) -> bool {
		true
	}

	fn log(&self, record: &log::Record<'_>) {
		println!("{}: {}: {}", record.target(), record.level(), record.args());
	}

	fn flush(&self) {}
}

static LOGGER: Logger = Logger;

fn main() {
	log::set_logger(&LOGGER).unwrap();
	let level: log::LevelFilter = if let Ok(value) = std::env::var("LOGLEVEL") {
		value.parse().unwrap()
	} else {
		log::LevelFilter::Info
	};
	log::set_max_level(level);
	let args: Vec<String> = std::env::args().collect();
	if args[1].ends_with(".git") || args[2].ends_with(".gpat") {
		sync_git_to_gpat(&args[1], &args[2]);
	} else if args[2].ends_with(".git") || args[1].ends_with(".gpat") {
		sync_gpat_to_git(&args[1], &args[2]);
	} else {
		panic!("Unknown format");
	}
}
