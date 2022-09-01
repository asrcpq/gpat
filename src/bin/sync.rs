use std::path::Path;

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let repo = if let Ok(repo) = git2::Repository::open(&args[1]) {
		repo
	} else if let Ok(repo) = git2::Repository::open_bare(&args[1]) {
		repo
	} else {
		panic!("Open fail {}", args[1])
	};
	let mut revwalk = repo.revwalk().unwrap();
	let sort = git2::Sort::TIME | git2::Sort::REVERSE;
	revwalk.set_sorting(sort).unwrap();
	revwalk.push_head().unwrap();
	for rev in revwalk {
		let commit = repo.find_commit(rev.unwrap()).unwrap();
		if commit.parents().len() >= 2 {
			panic!("Contains merge point: {}", commit.id());
		}
	}
	let mut revwalk = repo.revwalk().unwrap();
	revwalk.set_sorting(sort).unwrap();
	revwalk.push_head().unwrap();
	let mut prev_tree = None;
	let mut diff_options = git2::DiffOptions::new();
	diff_options.show_binary(true);
	for rev in revwalk {
		let rev = rev.unwrap();
		let commit = repo.find_commit(rev).unwrap();
		let object = repo.find_object(rev, None).unwrap();
		let tree = object.peel_to_tree().unwrap();
		let diff = repo.diff_tree_to_tree(
			prev_tree.as_ref(),
			Some(&tree),
			Some(&mut diff_options),
		).unwrap();
		let mut result: Vec<u8> = Vec::new();
		for idx in 0.. {
			let mut patch = if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, idx) {
				patch
			} else {
				break
			};
			let buf = patch.to_buf().unwrap();
			result.extend(buf.iter());
		}
		prev_tree = Some(tree);
		let time = commit.time().seconds();
		let path_string = format!("{}/{}.patch", args[2], time);
		let patch_path = Path::new(&path_string);
		if patch_path.exists() {
			let b = std::fs::read(patch_path).unwrap();
			if b == result {
				eprintln!("DEBUG: Check commit {} Ok", commit.id());
			} else {
				panic!("Check failed! maybe dup timestamp?");
			}
		} else {
			std::fs::write(patch_path, &result).unwrap();
			eprintln!("Write commit {}({} bytes) Ok", commit.id(), result.len());
		}
	}
}