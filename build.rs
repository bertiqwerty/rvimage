use std::process::Command;

fn git_cmd(args: &[&str]) -> Option<String> {
    Command::new("git").args(args).output().ok().map(|o|String::from_utf8(o.stdout).unwrap())
}

fn main() {
    let git_hash = git_cmd(&["rev-parse", "HEAD"]).unwrap_or("".to_string());
    let git_tag = git_cmd(&["tag", "HEAD"]).unwrap_or("".to_string());
    let is_dirty = git_cmd(&["diff"]).map(|o| o.trim().len() > 0) == Some(true);
    println!("cargo:rustc-env=GIT_HASH={git_hash}");
    println!("cargo:rustc-env=GIT_TAG={git_tag}");
    println!("cargo:rustc-env=GIT_DIRTY={is_dirty}");
}
