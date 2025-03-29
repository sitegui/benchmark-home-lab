use clap::Parser;
use std::fmt::Debug;
use std::net::{IpAddr, SocketAddr};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::{io, try_join};

#[derive(Parser)]
enum Cli {
    Benchmark {
        #[clap(long)]
        files: Vec<String>,
        #[clap(long, default_value_t = 30.0)]
        transcode_seconds: f64,
        #[clap(long, default_value_t = 1144)]
        port: u16,
        #[clap(long)]
        ip: IpAddr,
        #[clap(long, default_value_t = 5)]
        iterations: i32,
    },
    RemoteServer {
        #[clap(long, default_value_t = 1144)]
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli {
        Cli::Benchmark {
            files,
            transcode_seconds,
            port,
            ip,
            iterations,
        } => {
            benchmark(files, transcode_seconds, port, ip, iterations).await;
        }
        Cli::RemoteServer { port } => {
            remote_server(port).await;
        }
    }
}

async fn benchmark(
    file_paths: Vec<String>,
    transcode_seconds: f64,
    port: u16,
    ip: IpAddr,
    iterations: i32,
) {
    let transcode_duration = Duration::from_secs_f64(transcode_seconds);
    let transfer_address = SocketAddr::new(ip, port);

    for file_path in file_paths {
        println!("Benchmark with {}", file_path);

        time("Read file", iterations, || read(&file_path)).await;

        time("Transcoded file", iterations, || {
            transcode(&file_path, transcode_duration)
        })
        .await;

        time("Transferred data in LAN", iterations, || {
            transfer(&file_path, transfer_address)
        })
        .await;
    }
}

async fn remote_server(port: u16) {
    let server = TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("failed to bind");
    println!("Listening on {}", port);

    loop {
        let (connection, address) = server.accept().await.expect("failed to accept connection");
        println!("Got connection from {}", address);

        tokio::spawn(async move {
            let (reader, mut writer) = connection.into_split();
            let hash = hash(reader).await;
            writer.write_u8(hash).await.expect("failed to write hash");
            println!("Finished connection from {}", address);
        });
    }
}

async fn read(file_path: &str) -> u8 {
    hash(File::open(file_path).await.expect("failed to open file")).await
}

async fn transcode(file_path: &str, duration: Duration) -> u8 {
    let mut child = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-t",
            &duration.as_secs_f64().to_string(),
            "-i",
            file_path,
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-r",
            "30",
            "-crf",
            "26",
            "-f",
            "matroska",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ffmpeg process");

    let mut child_err = child.stderr.take().expect("failed to open ffmpeg stderr");
    let read_err_future = tokio::spawn(async move {
        let mut child_err_str = String::new();
        child_err
            .read_to_string(&mut child_err_str)
            .await
            .expect("failed to read ffmpeg stderr");
        child_err_str
    });

    let child_out = child.stdout.take().expect("failed to open ffmpeg stdout");
    let hash_future = tokio::spawn(hash(child_out));

    let status_future =
        tokio::spawn(async move { child.wait().await.expect("ffmpeg failed to execute") });

    let (read_err, hash, status) =
        try_join!(read_err_future, hash_future, status_future).expect("failed to transcode");
    if !status.success() {
        panic!("failed to execute ffmpeg, got stderr:\n{}", read_err);
    }

    hash
}

async fn transfer(file_path: &str, address: SocketAddr) -> u8 {
    let connection = TcpStream::connect(address)
        .await
        .expect("failed to connect");

    let (mut reader, mut writer) = connection.into_split();

    let mut file = File::open(file_path).await.expect("failed to open file");
    io::copy(&mut file, &mut writer)
        .await
        .expect("failed to send file");
    drop(writer);

    reader.read_u8().await.expect("failed to read file")
}

async fn hash(mut reader: impl AsyncRead + Unpin) -> u8 {
    let mut buffer = [0; 1024];
    let mut hash = 0;
    loop {
        let bytes = reader.read(&mut buffer).await.expect("failed to read file");
        if bytes == 0 {
            break;
        }
        for &byte in buffer[..bytes].iter() {
            hash ^= byte;
        }
    }

    hash
}

async fn time<C, F, O>(name: &str, iterations: i32, f: C)
where
    C: Fn() -> F,
    F: Future<Output = O>,
    O: Debug,
{
    let mut samples = vec![];
    let mut output = None;
    for _ in 0..iterations {
        let start = Instant::now();
        output = Some(f().await);
        samples.push(start.elapsed().as_secs_f64());
    }

    let n = samples.len() as f64;
    let avg = samples.iter().sum::<f64>() / n;
    let variance = samples
        .iter()
        .map(|sample| (sample - avg).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    let std = variance.sqrt();

    println!(
        "{} in {:.1} ± {:.1} s (got {:?})",
        name,
        avg,
        std,
        output.expect("at least one iteration")
    );
}
