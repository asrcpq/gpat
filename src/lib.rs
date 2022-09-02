use std::path::{Path, PathBuf};

pub fn sync_git_to_gpat(src: &str, dst: &str) {
	let repo = if let Ok(repo) = git2::Repository::open(src) {
		repo
	} else if let Ok(repo) = git2::Repository::open_bare(src) {
		repo
	} else {
		panic!("Open fail {}", src)
	};
	let mut existing_patches = if !Path::new(dst).exists() {
		std::fs::create_dir(dst).unwrap();
		Vec::new()
	} else {
		std::fs::read_dir(dst).unwrap()
			.map(|x| {
				let x = x.unwrap();
				if !x.file_type()
					.unwrap()
					.is_file()
				{
					panic!("Dst contains invalid file {:?}", x);
				}
				let filename = x.file_name()
					.into_string()
					.expect(&format!("Dst contains non string file {:?}", x));
				filename.split('.').next().unwrap().parse::<i64>().unwrap()
			})
			.collect()
	};
	existing_patches.sort_unstable();
	let mut existing_patch_idx = 0;
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
		let path_string = format!("{}/{}.patch", dst, time);
		let patch_path = Path::new(&path_string);
		if existing_patch_idx < existing_patches.len() {
			if existing_patches[existing_patch_idx] != time {
				panic!("Time mismatch {} vs {}", existing_patches[existing_patch_idx], time);
			}
			let b = std::fs::read(patch_path).unwrap();
			if b != result {
				panic!("Check failed! maybe dup timestamp?");
			}
			existing_patch_idx += 1;
		} else {
			std::fs::write(patch_path, &result).unwrap();
			eprintln!("Write commit {}({} bytes) Ok", commit.id(), result.len());
		}
	}
	if existing_patch_idx != existing_patches.len() {
		panic!("Gpat directory is newer than git");
	}
}

fn check_ext(path: &Path) -> bool {
	let ext = if let Some(x) = path.extension() {
		x
	} else {
		return false
	};
	ext.to_str() == Some("patch")
}

pub fn sync_gpat_to_git(src: &str, dst: &str) {
	let mut paths: Vec<(i64, PathBuf)> = Vec::new();
	for entry in std::fs::read_dir(src).unwrap() {
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
	let dst_path = Path::new(dst);
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
