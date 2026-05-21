use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;

pub fn run_install<F>(commands: Vec<Vec<String>>, on_line: F) -> Result<i32, String>
where
    F: Fn(&str, bool) + Send + Sync + 'static,
{
    let on_line = Arc::new(on_line);
    let mut last_exit = 0;

    for argv in commands {
        if argv.is_empty() {
            continue;
        }
        let (program, args) = argv.split_first().unwrap();

        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn {program}: {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "child stdout unavailable".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "child stderr unavailable".to_string())?;

        let stdout_cb = Arc::clone(&on_line);
        let stderr_cb = Arc::clone(&on_line);

        let stdout_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                stdout_cb(&line, false);
            }
        });
        let stderr_handle = thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                stderr_cb(&line, true);
            }
        });

        let status = child
            .wait()
            .map_err(|e| format!("wait failed for {program}: {e}"))?;
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        last_exit = status.code().unwrap_or(-1);
        if last_exit != 0 {
            return Ok(last_exit);
        }
    }

    Ok(last_exit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn captures_stdout_lines_in_order() {
        let collected: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&collected);
        let exit = run_install(
            vec![vec![
                "printf".to_string(),
                "alpha\nbeta\ngamma\n".to_string(),
            ]],
            move |line, is_err| {
                sink.lock().unwrap().push((line.to_string(), is_err));
            },
        )
        .expect("install runs");
        assert_eq!(exit, 0);
        let lines = collected.lock().unwrap().clone();
        let stdout_only: Vec<String> = lines
            .iter()
            .filter(|(_, is_err)| !is_err)
            .map(|(l, _)| l.clone())
            .collect();
        assert_eq!(stdout_only, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn stops_on_first_nonzero_exit() {
        let collected: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&collected);
        let exit = run_install(
            vec![
                vec!["sh".into(), "-c".into(), "echo first; exit 7".into()],
                vec!["sh".into(), "-c".into(), "echo second".into()],
            ],
            move |line, _| {
                sink.lock().unwrap().push(line.to_string());
            },
        )
        .expect("install runs");
        assert_eq!(exit, 7);
        let lines = collected.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l == "first"));
        assert!(!lines.iter().any(|l| l == "second"));
    }

    #[test]
    fn captures_stderr_lines() {
        let collected: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&collected);
        let exit = run_install(
            vec![vec!["sh".into(), "-c".into(), "echo oops >&2".into()]],
            move |line, is_err| {
                sink.lock().unwrap().push((line.to_string(), is_err));
            },
        )
        .expect("install runs");
        assert_eq!(exit, 0);
        let lines = collected.lock().unwrap().clone();
        assert!(lines.iter().any(|(l, is_err)| l == "oops" && *is_err));
    }
}
