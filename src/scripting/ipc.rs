//! The IPC server: accepts requests on the session socket and executes them on the Slint
//! event loop.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value as Json, json};
use slint_interpreter::Value;
use slintlu::ipc::{Request, Response, Target};

use super::json::{from_json, to_json};
use super::{ShellHost, host};

const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

/// Removes the socket file when the shell exits.
pub struct Server {
    path: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Server {
    pub fn start(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if UnixStream::connect(path).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("another slintlu is already listening on {}", path.display()),
            ));
        }
        // A stale socket from a crashed shell.
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        log::info!("IPC listening on {}", path.display());

        std::thread::Builder::new().name("slintlu-ipc".into()).spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    if let Err(e) = serve(stream) {
                        log::debug!("IPC client: {e}");
                    }
                });
            }
        })?;
        Ok(Self { path: path.to_path_buf() })
    }
}

fn serve(stream: UnixStream) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => dispatch(request),
            Err(e) => Response::failure(format!("invalid request: {e}")),
        };
        let mut reply = serde_json::to_string(&response).map_err(io::Error::other)?;
        reply.push('\n');
        reader.get_mut().write_all(reply.as_bytes())?;
        line.clear();
    }
    Ok(())
}

/// Runs the request on the event loop and waits for its result.
fn dispatch(request: Request) -> Response {
    let (tx, rx) = mpsc::channel();
    let sent = slint::invoke_from_event_loop(move || {
        let response = host::with(|host| handle(host, request))
            .unwrap_or_else(|| Response::failure("the shell is not running"));
        let _ = tx.send(response);
    });
    if sent.is_err() {
        return Response::failure("the event loop has stopped");
    }
    rx.recv_timeout(REPLY_TIMEOUT).unwrap_or_else(|_| Response::failure("timed out"))
}

fn handle(host: &ShellHost, request: Request) -> Response {
    match request {
        Request::Ping => Response::success(json!({ "version": env!("CARGO_PKG_VERSION") })),
        Request::Reload => {
            if host.load() {
                Response::success(Json::Null)
            } else {
                Response::failure("the configuration has errors; see the shell's log")
            }
        }
        Request::Quit => {
            let _ = slint::quit_event_loop();
            Response::success(Json::Null)
        }
        Request::Windows => Response::success(Json::Array(
            host.windows()
                .into_iter()
                .map(|(component, output)| json!({ "component": component, "output": output }))
                .collect(),
        )),
        Request::Outputs => Response::success(Json::Array(
            slintlu::outputs()
                .into_iter()
                .map(|o| {
                    json!({
                        "id": o.id, "name": o.name, "description": o.description,
                        "x": o.x, "y": o.y, "width": o.width, "height": o.height,
                        "scale": o.scale_factor,
                    })
                })
                .collect(),
        )),
        Request::Get { target } => with_target(&target, |t| {
            host.global_property(t.global, t.member)
                .map(|v| to_json(&v))
                .ok_or_else(|| format!("no exported global property {target}"))
        }),
        Request::Set { target, value } => with_target(&target, |t| {
            host.set_global_everywhere(t.global, t.member, from_json(&value)).map(|()| Json::Null)
        }),
        Request::Toggle { target } => {
            with_target(&target, |t| match host.global_property(t.global, t.member) {
                Some(Value::Bool(current)) => host
                    .set_global_everywhere(t.global, t.member, Value::Bool(!current))
                    .map(|()| Json::Bool(!current)),
                Some(_) => Err(format!("{target} is not a bool")),
                None => Err(format!("no exported global property {target}")),
            })
        }
        Request::Call { target, args } => with_target(&target, |t| {
            let args: Vec<Value> = args.iter().map(from_json).collect();
            host.invoke_global(t.global, t.member, &args)
                .map(|v| to_json(&v))
                .ok_or_else(|| format!("no exported global callback or function {target}"))
        }),
    }
}

fn with_target(target: &str, f: impl FnOnce(Target<'_>) -> Result<Json, String>) -> Response {
    match Target::parse(target).and_then(f) {
        Ok(result) => Response::success(result),
        Err(e) => Response::failure(e),
    }
}
