use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

fn write_config(root: &Path) -> PathBuf {
    let dataset = root.join("dataset");
    let snapshots = root.join("snapshots");
    let ls_root = root.join("ls");
    fs::create_dir_all(&dataset).unwrap();
    fs::create_dir_all(&snapshots).unwrap();
    fs::create_dir_all(&ls_root).unwrap();

    let config_path = root.join("config.toml");
    let contents = format!(
        "[paths]\n
dataset = \"{}\"\n\nsnapshots = \"{}\"\n\nls_root = \"{}\"\n",
        dataset.display(),
        snapshots.display(),
        ls_root.display()
    );
    fs::write(&config_path, contents).unwrap();
    config_path
}

fn write_manifest(ls_root: &Path, lines: &[String]) {
    let manifest_dir = ls_root.join("manifests");
    fs::create_dir_all(&manifest_dir).unwrap();
    let manifest_path = manifest_dir.join("snapshots_v2.tsv");
    let mut body = String::from("ts\tlabel\ttype\tparent\tbytes\tsha256\tlocal_path\tobject_key\n");
    for line in lines {
        body.push_str(line);
        body.push('\n');
    }
    fs::write(manifest_path, body).unwrap();
}

#[test]
fn restore_plan_includes_anchor_and_incremental() {
    let tmp = tempdir().unwrap();
    let config_path = write_config(tmp.path());
    let ls_root = tmp.path().join("ls");

    let anchor_path = ls_root
        .join("artifacts/anchors/dev@2024-01.full.send.zst.age");
    let incr_path = ls_root
        .join("artifacts/incr/dev@2024-02.incr.from_2024-01.send.zst.age");

    fs::create_dir_all(anchor_path.parent().unwrap()).unwrap();
    fs::create_dir_all(incr_path.parent().unwrap()).unwrap();

    let anchor_line = format!(
        "2024-01-01T00:00:00Z\t2024-01\tanchor\t\t1\tdeadbeef\t{}\t",
        anchor_path.display()
    );
    let incr_line = format!(
        "2024-02-01T00:00:00Z\t2024-02\tincremental\t2024-01\t2\tbeadfeed\t{}\t",
        incr_path.display()
    );

    write_manifest(&ls_root, &[anchor_line, incr_line]);

    let output = Command::new(env!("CARGO_BIN_EXE_dev-backup"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "restore",
            "plan",
            "2024-02",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec![anchor_path.to_str().unwrap(), incr_path.to_str().unwrap()]);
}

#[test]
fn restore_plan_stops_when_parent_snapshot_present() {
    let tmp = tempdir().unwrap();
    let config_path = write_config(tmp.path());
    let ls_root = tmp.path().join("ls");

    let anchor_path = ls_root
        .join("artifacts/anchors/dev@2024-01.full.send.zst.age");
    let incr_path = ls_root
        .join("artifacts/incr/dev@2024-02.incr.from_2024-01.send.zst.age");

    fs::create_dir_all(anchor_path.parent().unwrap()).unwrap();
    fs::create_dir_all(incr_path.parent().unwrap()).unwrap();

    let anchor_line = format!(
        "2024-01-01T00:00:00Z\t2024-01\tanchor\t\t1\tdeadbeef\t{}\t",
        anchor_path.display()
    );
    let incr_line = format!(
        "2024-02-01T00:00:00Z\t2024-02\tincremental\t2024-01\t2\tbeadfeed\t{}\t",
        incr_path.display()
    );

    write_manifest(&ls_root, &[anchor_line, incr_line]);

    let parent_snapshot_dir = ls_root.join("restore/snapshots/dev@2024-01");
    fs::create_dir_all(&parent_snapshot_dir).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_dev-backup"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "restore",
            "plan",
            "2024-02",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec![incr_path.to_str().unwrap()]);
}
