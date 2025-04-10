fn main() {
    match get_git_commit_hash_result() {
        Ok(result) => println!("cargo::rustc-env=KSYNTH_BUILD_GIT_COMMIT_HASH={result}"),
        Err(_) => println!("cargo::rustc-env=KSYNTH_BUILD_GIT_COMMIT_HASH={}", "Failed to retrieve the commit hash."),
    }
}

fn get_git_commit_hash_result() -> Result<String, Box<dyn std::error::Error>> {
    let repo = gix::discover(".")?;
    let commit_hash = repo.head_commit()?.id().to_string();

    Ok(commit_hash)
}
