use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const APP_NAME: &str = "desktop_manager";
const RES_DIR: &str = "resources";

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let blueprints = find_blueprints(Path::new(RES_DIR));

    // Need to tell cargo to rerun the build script when the blueprints are updated
    for blueprint in blueprints.iter() {
        println!(
            "cargo:rerun-if-changed={}",
            blueprint.to_str().unwrap_or_else(|| panic!(
                "Blueprint path is not valid UTF-8: {}",
                blueprint.to_string_lossy()
            ))
        );
    }

    if !blueprints.is_empty() {
        // Compile the blueprints
        let status = Command::new("blueprint-compiler")
            .arg("batch-compile")
            .arg(format!("{out_dir}/{RES_DIR}"))
            .arg(RES_DIR)
            .args(
                blueprints
                    .into_iter()
                    .map(|blp| blp.to_str().unwrap().to_string()),
            )
            .spawn()
            .expect("Failed to run blueprint compiler")
            .wait()
            .expect("Blueprint compiler wasn't running");

        if !status.success() {
            panic!("Failed to compile blueprints");
        }
    }

    glib_build_tools::compile_resources(
        &[format!("{out_dir}/{RES_DIR}"), RES_DIR.to_string()],
        format!("{RES_DIR}/resources.gresource.xml").as_str(),
        format!("{APP_NAME}.gresource").as_str(),
    );
}

fn find_blueprints(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "blp") {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}
