use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use slipbox_core::{GitBranch, RemoteUrl};
use tempfile::TempDir;

use super::*;

#[test]
fn smart_http_clones_updates_and_keeps_the_request_bounded() {
    let fixture = Fixture::new((FETCH_DEPTH + 8) as usize);
    let destination = fixture.root.path().join("client.git");
    let request = request(&destination);
    let cancelled = AtomicBool::new(false);
    let mut progress = Vec::new();

    let cloned = synchronize_url(
        &request,
        &fixture.server.url("remote.git"),
        true,
        None,
        &cancelled,
        |event| progress.push(event),
    )
    .expect("public smart-HTTP clone");

    assert_eq!(cloned.disposition, FetchDisposition::Cloned);
    assert_eq!(cloned.revision, fixture.head());
    assert_eq!(
        progress.iter().map(|event| event.stage).collect::<Vec<_>>(),
        [
            ProgressStage::Preparing,
            ProgressStage::Connecting,
            ProgressStage::Receiving,
            ProgressStage::Complete,
        ]
    );
    assert!(destination.join("shallow").is_file());
    let retained = git_stdout([
        "--git-dir",
        text(&destination),
        "rev-list",
        "--count",
        "refs/remotes/origin/main",
    ]);
    assert_eq!(
        retained.trim().parse::<u32>().expect("commit count"),
        FETCH_DEPTH
    );
    assert_eq!(
        git_stdout([
            "--git-dir",
            text(&destination),
            "for-each-ref",
            "--format=%(refname)",
        ])
        .lines()
        .collect::<Vec<_>>(),
        [
            "refs/heads/main",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ]
    );

    fixture.commit("updated");
    let updated = synchronize_url(
        &request,
        &fixture.server.url("remote.git"),
        true,
        None,
        &cancelled,
        |_| {},
    )
    .expect("smart-HTTP update");
    assert_eq!(updated.disposition, FetchDisposition::Updated);
    assert_eq!(updated.revision, fixture.head());

    let unchanged = synchronize_url(
        &request,
        &fixture.server.url("remote.git"),
        true,
        None,
        &cancelled,
        |_| {},
    )
    .expect("smart-HTTP no-op fetch");
    assert_eq!(unchanged.disposition, FetchDisposition::Unchanged);
    assert_eq!(unchanged.received_objects, 0);
}

#[test]
fn cancellation_and_failures_publish_no_sensitive_diagnostics() {
    let fixture = Fixture::new(1);
    let destination = fixture.root.path().join("cancelled.git");
    let request = request(&destination);
    let cancelled = AtomicBool::new(true);
    let requests_before = fixture.server.requests();

    assert_eq!(
        synchronize_url(
            &request,
            &fixture.server.url("remote.git"),
            true,
            None,
            &cancelled,
            |_| {},
        ),
        Err(GitError::Cancelled)
    );
    assert_eq!(fixture.server.requests(), requests_before);
    assert!(!destination.exists());

    let failed = synchronize_url(
        &request,
        &fixture.server.url("missing.git"),
        true,
        None,
        &AtomicBool::new(false),
        |_| {},
    )
    .expect_err("a missing smart-HTTP repository");
    let diagnostic = format!("{failed:?} {failed}");
    assert!(!diagnostic.contains("missing.git"));
    assert!(!diagnostic.contains("127.0.0.1"));
    assert!(!destination.exists());

    let secret = "github_pat_synthetic_secret_material";
    let token = AccessToken::new(secret.as_bytes().to_vec()).expect("synthetic token");
    assert_eq!(format!("{token:?}"), "AccessToken(redacted)");
    assert!(!format!("{token:?}").contains(secret));

    let foreign = FetchRequest::new(
        RemoteUrl::parse("https://example.test/private.git").expect("foreign remote"),
        GitBranch::parse("main").expect("test branch"),
        fixture.root.path().join("foreign.git"),
    )
    .expect("foreign request");
    assert_eq!(
        synchronize(
            &foreign,
            Some(AccessToken::new(secret.as_bytes().to_vec()).expect("synthetic token")),
            &AtomicBool::new(false),
            |_| {},
        ),
        Err(GitError::CredentialRefused)
    );
    assert!(!foreign.repository().exists());

    let non_default_port = FetchRequest::new(
        RemoteUrl::parse("https://github.com:444/owner/private.git")
            .expect("GitHub remote on a non-default port"),
        GitBranch::parse("main").expect("test branch"),
        fixture.root.path().join("non-default-port.git"),
    )
    .expect("non-default-port request");
    assert_eq!(
        synchronize(
            &non_default_port,
            Some(AccessToken::new(secret.as_bytes().to_vec()).expect("synthetic token")),
            &AtomicBool::new(false),
            |_| {},
        ),
        Err(GitError::CredentialRefused)
    );
    assert!(!non_default_port.repository().exists());
}

#[test]
fn an_in_flight_smart_http_clone_can_be_cancelled() {
    let fixture = Fixture::new(32);
    fixture.server.hold_upload();
    let destination = fixture.root.path().join("interrupted.git");
    let request = request(&destination);
    let remote = fixture.server.url("remote.git");
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let worker = thread::spawn(move || {
        synchronize_url(&request, &remote, true, None, &worker_cancelled, |_| {})
    });

    fixture.server.await_held_upload();
    cancelled.store(true, Ordering::Relaxed);
    fixture.server.release_upload();

    assert_eq!(
        worker.join().expect("clone worker"),
        Err(GitError::Cancelled)
    );
    assert!(!destination.exists());
}

fn request(repository: &Path) -> FetchRequest {
    FetchRequest::new(
        RemoteUrl::parse("https://example.test/remote.git").expect("test remote"),
        GitBranch::parse("main").expect("test branch"),
        repository.to_path_buf(),
    )
    .expect("test fetch request")
}

struct Fixture {
    root: TempDir,
    seed: PathBuf,
    server: SmartHttp,
}

impl Fixture {
    fn new(commits: usize) -> Self {
        let root = tempfile::tempdir().expect("fixture root");
        let remote = root.path().join("remote.git");
        let seed = root.path().join("seed");
        run_git(["init", "--bare", text(&remote)]);
        run_git(["init", text(&seed)]);
        run_git_in(&seed, ["config", "user.name", "Slipbox Fixture"]);
        run_git_in(&seed, ["config", "user.email", "fixture@example.invalid"]);
        run_git_in(&seed, ["config", "gc.auto", "0"]);
        run_git_in(&seed, ["config", "maintenance.auto", "false"]);
        run_git_in(&seed, ["branch", "-M", "main"]);
        run_git_in(&seed, ["remote", "add", "origin", text(&remote)]);
        for serial in 0..commits {
            fs::write(seed.join("note.org"), format!("* Note {serial}\n"))
                .expect("write fixture note");
            run_git_in(&seed, ["add", "note.org"]);
            run_git_in(&seed, ["commit", "-m", &format!("note {serial}")]);
        }
        run_git_in(&seed, ["tag", "not-fetched"]);
        run_git_in(&seed, ["push", "origin", "main", "--tags"]);
        run_git([
            "--git-dir",
            text(&remote),
            "symbolic-ref",
            "HEAD",
            "refs/heads/main",
        ]);
        let server = SmartHttp::start(root.path().to_path_buf());
        Self { root, seed, server }
    }

    fn commit(&self, value: &str) {
        fs::write(self.seed.join("note.org"), format!("* {value}\n"))
            .expect("write updated fixture note");
        run_git_in(&self.seed, ["add", "note.org"]);
        run_git_in(&self.seed, ["commit", "-m", value]);
        run_git_in(&self.seed, ["push", "origin", "main"]);
    }

    fn head(&self) -> String {
        git_stdout_in(&self.seed, ["rev-parse", "HEAD"])
            .trim()
            .to_owned()
    }
}

struct SmartHttp {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
    hold_upload: Arc<AtomicBool>,
    upload_ready: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl SmartHttp {
    fn start(project_root: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind smart-HTTP fixture");
        let address = listener.local_addr().expect("fixture address");
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let hold_upload = Arc::new(AtomicBool::new(false));
        let upload_ready = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_requests = Arc::clone(&requests);
        let worker_hold = Arc::clone(&hold_upload);
        let worker_ready = Arc::clone(&upload_ready);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let Ok((stream, _)) = listener.accept() else {
                    continue;
                };
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                worker_requests.fetch_add(1, Ordering::Relaxed);
                serve(
                    stream,
                    &project_root,
                    address.port(),
                    &worker_hold,
                    &worker_ready,
                );
            }
        });
        Self {
            address,
            stop,
            requests,
            hold_upload,
            upload_ready,
            worker: Some(worker),
        }
    }

    fn url(&self, repository: &str) -> String {
        format!("http://{}/{repository}", self.address)
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }

    fn hold_upload(&self) {
        self.hold_upload.store(true, Ordering::Relaxed);
    }

    fn await_held_upload(&self) {
        for _ in 0..1_000 {
            if self.upload_ready.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("the fixture never reached an upload response");
    }

    fn release_upload(&self) {
        self.hold_upload.store(false, Ordering::Relaxed);
    }
}

impl Drop for SmartHttp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.hold_upload.store(false, Ordering::Relaxed);
        if let Ok(stream) = TcpStream::connect(self.address) {
            let _ = stream.shutdown(Shutdown::Both);
        }
        if let Some(worker) = self.worker.take() {
            worker.join().expect("smart-HTTP fixture stopped cleanly");
        }
    }
}

fn serve(
    mut stream: TcpStream,
    project_root: &Path,
    port: u16,
    hold_upload: &AtomicBool,
    upload_ready: &AtomicBool,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("fixture read timeout");
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let mut command = Command::new(git_program());
    command
        .arg("http-backend")
        .env("GIT_PROJECT_ROOT", project_root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", &request.path)
        .env("QUERY_STRING", &request.query)
        .env("REQUEST_METHOD", &request.method)
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("SERVER_NAME", "127.0.0.1")
        .env("SERVER_PORT", port.to_string())
        .env("REMOTE_ADDR", "127.0.0.1")
        .env("CONTENT_LENGTH", request.body.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(content_type) = request.content_type {
        command.env("CONTENT_TYPE", content_type);
    }
    if let Some(protocol) = request.git_protocol {
        command.env("HTTP_GIT_PROTOCOL", protocol);
    }
    let mut child = command.spawn().expect("start git http-backend");
    child
        .stdin
        .take()
        .expect("backend stdin")
        .write_all(&request.body)
        .expect("write backend request");
    let output = child.wait_with_output().expect("read backend response");
    let (headers, body) = split_headers(&output.stdout).expect("CGI response headers");
    let mut status = "200 OK";
    let mut forwarded = Vec::new();
    for line in headers.lines() {
        if let Some(value) = line.strip_prefix("Status: ") {
            status = value;
        } else if !line.is_empty() {
            forwarded.push(line);
        }
    }
    write!(stream, "HTTP/1.1 {status}\r\n").expect("write fixture status");
    for header in forwarded {
        write!(stream, "{header}\r\n").expect("write fixture header");
    }
    write!(
        stream,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write fixture framing");
    if request.path.ends_with("/git-upload-pack") && hold_upload.load(Ordering::Relaxed) {
        upload_ready.store(true, Ordering::Relaxed);
        while hold_upload.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(5));
        }
    }
    stream.write_all(body).expect("write fixture body");
}

struct HttpRequest {
    method: String,
    path: String,
    query: String,
    content_type: Option<String>,
    git_protocol: Option<String>,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<HttpRequest> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8 * 1024];
    let header_end = loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > 1024 * 1024 {
            return None;
        }
        if let Some(end) = find_bytes(&bytes, b"\r\n\r\n") {
            break end + 4;
        }
    };
    let (method, path, query, length, content_type, git_protocol) = {
        let headers = std::str::from_utf8(&bytes[..header_end - 4]).ok()?;
        let mut lines = headers.lines();
        let mut first = lines.next()?.split_whitespace();
        let method = first.next()?.to_owned();
        let target = first.next()?;
        let (path, query) = target
            .split_once('?')
            .map_or((target, ""), |(path, query)| (path, query));
        let mut length = 0usize;
        let mut content_type = None;
        let mut git_protocol = None;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().ok()?;
            } else if name.eq_ignore_ascii_case("content-type") {
                content_type = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("git-protocol") {
                git_protocol = Some(value.to_owned());
            }
        }
        (
            method,
            path.to_owned(),
            query.to_owned(),
            length,
            content_type,
            git_protocol,
        )
    };
    while bytes.len() < header_end + length {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Some(HttpRequest {
        method,
        path,
        query,
        content_type,
        git_protocol,
        body: bytes[header_end..header_end + length].to_vec(),
    })
}

fn split_headers(response: &[u8]) -> Option<(String, &[u8])> {
    let boundary = find_bytes(response, b"\r\n\r\n")?;
    let headers = String::from_utf8(response[..boundary].to_vec()).ok()?;
    Some((headers, &response[boundary + 4..]))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn git_program() -> PathBuf {
    let output = Command::new("which")
        .arg("git")
        .output()
        .expect("find git for the fixture server");
    assert!(output.status.success(), "the fixture needs host git");
    PathBuf::from(
        String::from_utf8(output.stdout)
            .expect("git path is UTF-8")
            .trim(),
    )
}

fn run_git<const N: usize>(arguments: [&str; N]) {
    let output = Command::new(git_program())
        .args(arguments)
        .output()
        .expect("run fixture git");
    assert!(
        output.status.success(),
        "fixture git command {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_git_in<const N: usize>(directory: &Path, arguments: [&str; N]) {
    let output = Command::new(git_program())
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run fixture git");
    assert!(
        output.status.success(),
        "fixture git command {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout<const N: usize>(arguments: [&str; N]) -> String {
    let output = Command::new(git_program())
        .args(arguments)
        .output()
        .expect("read fixture git output");
    assert!(output.status.success(), "fixture git command succeeded");
    String::from_utf8(output.stdout).expect("fixture output is UTF-8")
}

fn git_stdout_in<const N: usize>(directory: &Path, arguments: [&str; N]) -> String {
    let output = Command::new(git_program())
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("read fixture git output");
    assert!(output.status.success(), "fixture git command succeeded");
    String::from_utf8(output.stdout).expect("fixture output is UTF-8")
}

fn text(path: &Path) -> &str {
    path.to_str().expect("fixture path is UTF-8")
}
