use gpat::{sync_git_to_gpat, sync_gpat_to_git};

fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args[1].ends_with(".git") || args[2].ends_with(".gpat") {
		sync_git_to_gpat(&args[1], &args[2]);
	} else if args[2].ends_with(".git") || args[1].ends_with(".gpat") {
		sync_gpat_to_git(&args[1], &args[2]);
	} else {
		panic!("Unknown format");
	}
}
