use super::{Container, ContainerError};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const PATH_ENV: &str = "RELIQUARY_PROCESS_LEASE_PATH";
const MODE_ENV: &str = "RELIQUARY_PROCESS_LEASE_MODE";

fn test_path(label: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
    std::fs::create_dir_all(&root).unwrap();
    root.join(format!("{label}-{}.cva", uuid::Uuid::new_v4()))
}

fn spawn_holder(path: &std::path::Path, mode: &str) -> (Child, ChildStdin, Receiver<String>) {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("container::file_lease_process_tests::lease_child_holds_requested_lease")
        .arg("--nocapture")
        .env(PATH_ENV, path)
        .env(MODE_ENV, mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = sender.send(line);
                    break;
                }
                Ok(_) if line.trim() == "LEASE_READY" => {
                    let _ = sender.send(line);
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = sender.send(error.to_string());
                    break;
                }
            }
        }
    });
    let stdin = child.stdin.take().unwrap();
    (child, stdin, receiver)
}

fn wait_ready(child: &mut Child, receiver: &Receiver<String>) {
    match receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(line) => assert_eq!(line.trim(), "LEASE_READY"),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not acquire its lease and signal readiness: {error}");
        }
    }
}

fn kill_holder(child: &mut Child, _stdin: ChildStdin) {
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
}

#[test]
fn child_writer_excludes_local_opens_and_process_exit_releases_the_lock() {
    let path = test_path("process-writer");
    drop(Container::create(&path).unwrap());
    let (mut child, stdin, ready) = spawn_holder(&path, "writer");
    wait_ready(&mut child, &ready);

    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::WriterConflict)
    ));
    assert!(matches!(
        Container::open_read_only_for_scan(&path),
        Err(ContainerError::WriterConflict)
    ));
    kill_holder(&mut child, stdin);
    assert!(Container::open(&path).is_ok());
    let _ = std::fs::remove_file(path);
}

#[test]
fn child_shared_reader_excludes_writer_but_allows_another_reader() {
    let path = test_path("process-reader");
    drop(Container::create(&path).unwrap());
    let (mut child, stdin, ready) = spawn_holder(&path, "reader");
    wait_ready(&mut child, &ready);

    assert!(matches!(
        Container::open(&path),
        Err(ContainerError::WriterConflict)
    ));
    assert!(Container::open_read_only_for_scan(&path).is_ok());
    kill_holder(&mut child, stdin);
    assert!(Container::open(&path).is_ok());
    let _ = std::fs::remove_file(path);
}

#[test]
fn cross_process_writer_lock_is_shared_by_hardlink_and_symlink_aliases() {
    let path = test_path("process-alias");
    let hardlink = path.with_file_name(format!("{}-hardlink.cva", uuid::Uuid::new_v4()));
    let symlink = path.with_file_name(format!("{}-symlink.cva", uuid::Uuid::new_v4()));
    drop(Container::create(&path).unwrap());
    if let Err(error) = std::fs::hard_link(&path, &hardlink) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            eprintln!("skipping hardlink alias assertion: Windows denied hardlink creation");
            let (mut child, stdin, ready) = spawn_holder(&path, "writer");
            wait_ready(&mut child, &ready);
            assert!(matches!(
                Container::open(&path),
                Err(ContainerError::WriterConflict)
            ));
            kill_holder(&mut child, stdin);
            let _ = std::fs::remove_file(path);
            return;
        }
        panic!("create hardlink alias: {error}");
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(&path, &symlink).unwrap();
    #[cfg(windows)]
    match std::os::windows::fs::symlink_file(&path, &symlink) {
        Ok(()) => {}
        Err(error)
            if error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314) =>
        {
            eprintln!("skipping symlink alias assertion: Windows denied symlink privilege");
        }
        Err(error) => panic!("create symlink alias: {error}"),
    }

    let (mut child, stdin, ready) = spawn_holder(&path, "writer");
    wait_ready(&mut child, &ready);
    assert!(matches!(
        Container::open(&hardlink),
        Err(ContainerError::WriterConflict)
    ));
    #[cfg(unix)]
    assert!(matches!(
        Container::open(&symlink),
        Err(ContainerError::WriterConflict)
    ));
    #[cfg(windows)]
    if symlink.exists() {
        assert!(matches!(
            Container::open(&symlink),
            Err(ContainerError::WriterConflict)
        ));
    }
    kill_holder(&mut child, stdin);
    assert!(Container::open(&hardlink).is_ok());

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(hardlink);
    let _ = std::fs::remove_file(symlink);
}

#[test]
fn lease_child_holds_requested_lease() {
    let (Ok(path), Ok(mode)) = (std::env::var(PATH_ENV), std::env::var(MODE_ENV)) else {
        return;
    };
    let _container = match mode.as_str() {
        "writer" => Container::open(path).unwrap(),
        "reader" => Container::open_read_only_for_scan(path).unwrap(),
        _ => panic!("unknown lease child mode"),
    };
    println!("LEASE_READY");
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}
