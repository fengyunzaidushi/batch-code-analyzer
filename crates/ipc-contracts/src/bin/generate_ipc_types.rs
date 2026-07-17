use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process,
};

fn main() {
    let out_dir = generated_types_dir();
    let result = match env::args().nth(1).as_deref() {
        None => regenerate_types(&out_dir),
        Some("--check") => check_types_are_current(&out_dir),
        Some(argument) => Err(format!("unknown argument: {argument}").into()),
    };

    if let Err(error) = result {
        eprintln!("IPC type generation failed: {error}");
        process::exit(1);
    }
}

fn generated_types_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/ipc-types/src/generated")
}

fn regenerate_types(out_dir: &Path) -> Result<(), Box<dyn Error>> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }

    fs::create_dir_all(out_dir)?;
    batch_code_analyzer_ipc_contracts::export_types(out_dir)?;

    Ok(())
}

fn check_types_are_current(out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let temporary_dir = env::temp_dir().join(format!(
        "batch-code-analyzer-ipc-types-check-{}",
        process::id()
    ));
    regenerate_types(&temporary_dir)?;

    let expected = generated_files(&temporary_dir)?;
    let actual = if out_dir.exists() {
        generated_files(out_dir)?
    } else {
        BTreeMap::new()
    };
    fs::remove_dir_all(&temporary_dir)?;

    if expected != actual {
        return Err("generated IPC types are stale; run `pnpm ipc:generate`".into());
    }

    Ok(())
}

fn generated_files(out_dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut files = BTreeMap::new();

    for entry in fs::read_dir(out_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.insert(
                entry.file_name().to_string_lossy().into_owned(),
                fs::read(entry.path())?,
            );
        }
    }

    Ok(files)
}
