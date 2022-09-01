// reconstruct git repo from patches
use std::path::{Path, PathBuf};

fn check_ext(path: &Path) -> bool {
	let ext = if let Some(x) = path.extension() {
		x
	} else {
		return false
	};
	ext.to_str() == Some("patch")
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let mut paths: Vec<(i64, PathBuf)> = Vec::new();
	for entry in std::fs::read_dir(&args[1]).unwrap() {
		let entry = entry.unwrap();
		let path = entry.path();
		if !check_ext(&path) {
			eprintln!("invalid file {:?}", path);
			continue
		}
		let filename = if let Ok(s) = entry.file_name().into_string() {
			s
		} else {
			eprintln!("Filename not string {:?}", path);
			continue
		};
		let mut iter = filename.split('.');
		let t = if let Ok(t) = iter.next().unwrap().parse::<i64>() {
			t
		} else {
			eprintln!("Filename not epoch {:?}", path);
			continue
		};
		paths.push((t, path.into()));
	}
	paths.sort_unstable();
	let dst_path = Path::new(&args[2]);
	if dst_path.exists() {
		if std::fs::read_dir(dst_path).unwrap().next().is_some() {
			panic!("Dst non-empty");
		}
	}
	let repo = git2::Repository::init_bare(dst_path).unwrap();
	let mut last_commit_oid = None;
	for (epoch, path) in paths.into_iter() {
		let sig = git2::Signature::new("idfc", "idfc", &git2::Time::new(epoch, 0)).unwrap();
		let buffer = std::fs::read(path).unwrap();
		let diff = git2::Diff::from_buffer(&buffer).unwrap();
		repo.apply(&diff, git2::ApplyLocation::Index, None).unwrap();
		let tree_oid = repo.index().unwrap().write_tree().unwrap();
		let tree = repo.find_tree(tree_oid).unwrap();
		let commit_oid = if let Some(last_commit_oid) = last_commit_oid {
			let last_commit = repo.find_commit(last_commit_oid).unwrap();
			repo.commit(
				None,
				&sig,
				&sig,
				"",
				&tree,
				&[&last_commit],
			).unwrap()
		} else {
			repo.commit(
				None,
				&sig,
				&sig,
				"",
				&tree,
				&[],
			).unwrap()
		};
		last_commit_oid = Some(commit_oid);
		eprintln!("Commit {:?}", commit_oid);
	}
	let commit = repo.find_commit(last_commit_oid.unwrap()).unwrap();
	repo.branch("master", &commit, false).unwrap();
}
