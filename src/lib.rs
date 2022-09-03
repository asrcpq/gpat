use std::path::Path;

fn get_revwalk(repo: &git2::Repository) -> git2::Revwalk {
	let mut revwalk = repo.revwalk().unwrap();
	let sort = git2::Sort::TIME | git2::Sort::REVERSE;
	revwalk.set_sorting(sort).unwrap();
	revwalk.push_head().unwrap();
	revwalk
}

fn parent_check(repo: &git2::Repository) {
	let mut revwalk = get_revwalk(repo);
	let mut last_commit;
	if let Some(rev) = revwalk.next() {
		let commit = repo.find_commit(rev.unwrap()).unwrap();
		assert_eq!(commit.parents().len(), 0);
		last_commit = commit.id();
	} else {
		return
	}
	for rev in revwalk {
		let commit = repo.find_commit(rev.unwrap()).unwrap();
		let mut parents = commit.parent_ids();
		if parents.len() != 1 {
			panic!("parents != 1, merge point? {}", commit.id());
		}
		let id = parents.next().unwrap();
		assert_eq!(id, last_commit);
		last_commit = commit.id();
	}
}

fn open_git_repo(path: &str) -> Option<git2::Repository> {
	let path = Path::new(path);
	if path.exists() {
		if std::fs::read_dir(path).unwrap().next().is_some() {
			let repo = if let Ok(repo) = git2::Repository::open(path) {
				repo
			} else if let Ok(repo) = git2::Repository::open_bare(path) {
				repo
			} else {
				panic!("Open fail {:?}", path)
			};
			if !repo.is_empty().unwrap() {
				parent_check(&repo);
			}
			return Some(repo)
		}
	}
	None
}

fn get_gpat_patch_list(dst: &str) -> Vec<i64> {
	let mut patch_list = if !Path::new(dst).exists() {
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
				let mut name_split_iter = filename.split('.');
				let id = name_split_iter.next().unwrap().parse::<i64>().unwrap();
				assert_eq!(name_split_iter.next().unwrap(), "patch");
				id
			})
			.collect()
	};
	patch_list.sort_unstable();
	patch_list
}

pub fn sync_git_to_gpat(src: &str, dst: &str) {
	let mut commit_count = 0;

	let repo = open_git_repo(src).unwrap();
	let existing_patches = get_gpat_patch_list(dst);
	let mut existing_patch_idx = 0;
	if repo.is_empty().unwrap() {
		panic!("Git repo is empty");
	}
	let revwalk = get_revwalk(&repo);
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
			log::debug!("Write commit {}({} bytes) Ok", commit.id(), result.len());
			commit_count += 1;
		}
	}
	if existing_patch_idx != existing_patches.len() {
		panic!("Gpat directory is newer than git");
	}
	log::info!("Summary: existing {}, commit {}", existing_patches.len(), commit_count);
}

pub fn sync_gpat_to_git(src: &str, dst: &str) {
	// skip, commit
	let mut count = [0usize; 2];

	let patch_list = get_gpat_patch_list(src);
	let (repo, created) = if let Some(repo) = open_git_repo(dst) {
		log::info!("Existing repo found");
		(repo, false)
	} else {
		let repo = git2::Repository::init_bare(dst).unwrap();
		log::info!("Create new repo");
		(repo, true)
	};
	let revwalk = if created || repo.is_empty().unwrap() {
		None
	} else {
		Some(get_revwalk(&repo))
	};
	let mut revwalk = revwalk.into_iter().flatten();
	let mut last_commit_oid = None;
	for epoch in patch_list.into_iter() {
		if let Some(rev) = revwalk.next() {
			let rev = rev.unwrap();
			let commit = repo.find_commit(rev).unwrap();
			if commit.time().seconds() == epoch {
				// TODO: strict content check
				log::debug!("epoch match, continue");
				count[0] += 1;
				continue
			} else {
				panic!("ecoch mismatch: commit {}", commit.id());
			}
		}

		let sig = git2::Signature::new("idfc", "idfc", &git2::Time::new(epoch, 0)).unwrap();
		let buffer = std::fs::read(&format!("{}/{}.patch", src, epoch)).unwrap();
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
		count[1] += 1;
		log::debug!("Commit {:?}", commit_oid);
	}
	if revwalk.next().is_some() {
		panic!("Git repo is newer than gpat");
	}
	if let Some(commit_oid) = last_commit_oid {
		let commit = repo.find_commit(commit_oid).unwrap();
		repo.branch("master", &commit, false).unwrap();
		log::info!("Marked master")
	} else {
		log::info!("Not updated");
	}
	log::info!("Summary: skip {}, commit {}", count[0], count[1]);
}
