use std::process::Command;

fn git_cmd(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8(o.stdout).unwrap())
}

fn main() {
    let git_desc = git_cmd(&["describe"]).unwrap_or_default();
    let is_dirty = git_cmd(&["diff"]).map(|o| !o.trim().is_empty()) == Some(true);

    println!("cargo:rustc-env=GIT_DESC={git_desc}");
    println!("cargo:rustc-env=GIT_DIRTY={is_dirty}");
}
