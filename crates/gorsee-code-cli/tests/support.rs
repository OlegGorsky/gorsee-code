#![allow(dead_code)]

use std::{
    io::{Read, Write},
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use portable_pty::{native_pty_system, Child, CommandBuilder, ExitStatus, PtySize};

pub struct PtySession {
    child: Box<dyn Child + Send + Sync>,
    output: Arc<Mutex<String>>,
    reader: Option<thread::JoinHandle<()>>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    pub fn spawn_gcode(binary: &str, cwd: &Path) -> Self {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize::default()).unwrap();
        let mut command = CommandBuilder::new(binary);
        command.cwd(cwd);
        command.env_remove("NEUROGATE_API_KEY");
        command.env_remove("GORSEE_NEUROGATE_API_KEY");

        let child = pair.slave.spawn_command(command).unwrap();
        let reader = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let output = Arc::new(Mutex::new(String::new()));
        let reader = Some(spawn_reader(reader, output.clone()));

        Self {
            child,
            output,
            reader,
            writer,
        }
    }

    pub fn send(&mut self, input: &str) {
        self.writer.write_all(input.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }

    pub fn wait_for(&self, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.snapshot().contains(needle) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }

    pub fn finish(mut self) -> (ExitStatus, String) {
        let status = wait_for_child(&mut self.child);
        let output = self.output.clone();
        drop(self.writer);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let transcript = output.lock().unwrap().clone();
        (status, transcript)
    }

    fn snapshot(&self) -> String {
        self.output.lock().unwrap().clone()
    }
}

fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    output: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; 1024];
        while let Ok(bytes) = reader.read(&mut buffer) {
            if bytes == 0 {
                break;
            }
            output
                .lock()
                .unwrap()
                .push_str(&String::from_utf8_lossy(&buffer[..bytes]));
        }
    })
}

fn wait_for_child(child: &mut Box<dyn Child + Send + Sync>) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            return child.wait().unwrap();
        }
        thread::sleep(Duration::from_millis(20));
    }
}

pub fn assert_product_output(output: &str) {
    let lowered = output.to_lowercase();
    for forbidden in [
        word(&['f', 'o', 'u', 'n', 'd', 'a', 't', 'i', 'o', 'n']),
        word(&[
            'v', 'e', 'r', 't', 'i', 'c', 'a', 'l', ' ', 's', 'l', 'i', 'c', 'e',
        ]),
        word(&['f', 'i', 'x', 't', 'u', 'r', 'e']),
        word(&['s', 'c', 'a', 'f', 'f', 'o', 'l', 'd']),
        word(&['m', 'v', 'p']),
        word(&['m', 'i', 'n', 'i', 'm', 'a', 'l']),
        word(&['d', 'e', 'm', 'o']),
        word(&['p', 'l', 'a', 'c', 'e', 'h', 'o', 'l', 'd', 'e', 'r']),
        word(&['m', 'i', 's', 's', 'i', 'o', 'n']),
    ] {
        assert!(
            !lowered.contains(&forbidden),
            "output contained forbidden product wording {forbidden:?}: {output}"
        );
    }
}

fn word(chars: &[char]) -> String {
    chars.iter().collect()
}
