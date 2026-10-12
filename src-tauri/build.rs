use std::path::Path;
use std::process::Command;

fn git_output(directory: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("Invalid Git output: {error}"))
}

fn build_commit(directory: &Path) -> Result<String, String> {
    let reference = git_output(directory, &["rev-parse", "--symbolic-full-name", "HEAD"])?;
    for name in ["HEAD", "index", "packed-refs", reference.as_str()] {
        let path = git_output(directory, &["rev-parse", "--git-path", name])?;
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = git_output(directory, &["rev-parse", "--short=8", "HEAD"])?;
    let status = git_output(directory, &["status", "--porcelain", "--untracked-files=normal"])?;
    Ok(if status.is_empty() {
        commit
    } else {
        format!("{commit}-dirty")
    })
}

fn main() {
    let directory = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest directory is missing");
    let directory = Path::new(&directory);
    for path in [
        directory.join("build.rs"),
        directory.join("src"),
        directory.join("Cargo.toml"),
        directory.join("Cargo.lock"),
        directory.join("tauri.conf.json"),
        directory.join("..").join("src"),
        directory.join("..").join("vite.config.ts"),
        directory.join("..").join("package.json"),
        directory.join("..").join("package-lock.json"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let commit = match build_commit(directory) {
        Ok(commit) => commit,
        Err(error) => {
            println!("cargo:warning=Build commit unavailable: {error}");
            "unknown".to_string()
        }
    };
    println!("cargo:rustc-env=FLUXVOICE_BUILD_COMMIT={commit}");
    tauri_build::build()
}
