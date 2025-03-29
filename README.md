# Benchmark home lab

I wanted to compare the IO, computing and networking power of different home lab servers.

Candidates:

1. a Raspberry Pi 4 using external hard drive and ethernet
2. an old Dell Inspiron 7560 using internal SSD and ethernet
3. a tower PC using internal SSD and wifi

In another server in the LAN:

```
git clone https://github.com/sitegui/benchmark-home-lab.git
cd benchmark-home-lab
cargo build --release
hostname --all-ip-addresses | cut -d' ' -f1
./target/release/benchmark-home-lab echo-server
```

In each benchmark target:

```
git clone https://github.com/sitegui/benchmark-home-lab.git
cd benchmark-home-lab
cargo build --release
DATA_DIR=...
ECHO_IP=192.168.1.25
./target/release/benchmark-home-lab benchmark --files "$DATA_DIR/small.mp4" --files "$DATA_DIR/large.avi" --ip $ECHO_IP
```

## Results

Values in seconds, measured by running the operation 5 times and getting the average and sample standard deviation.

I've used two video files, one modern but small 10MiB and one old but large 1GiB.

```
% ffprobe small.mp4
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'data/small.mp4':
  Metadata:
    major_brand     : mp42
    minor_version   : 0
    compatible_brands: mp42isom
  Duration: 00:00:49.95, start: 0.000000, bitrate: 1507 kb/s
  Stream #0:0(und): Video: h264 (Baseline) (avc1 / 0x31637661), yuv420p(tv, bt709), 480x848, 1442 kb/s, 30 fps, 30 tbr, 600 tbn, 1200 tbc (default)
  Stream #0:1(und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 63 kb/s (default)

% ffprobe data/large.avi
Input #0, avi, from 'data/large.avi':
  Metadata:
    software        : FairUse Wizard - http://fairusewizard.com
  Duration: 01:17:28.00, start: 0.000000, bitrate: 1768 kb/s
  Stream #0:0: Video: mpeg4 (Advanced Simple Profile) (XVID / 0x44495658), yuv420p, 640x464 [SAR 1:1 DAR 40:29], 1630 kb/s, 25 fps, 25 tbr, 25 tbn, 25 tbc
  Stream #0:1: Audio: mp3 (U[0][0][0] / 0x0055), 48000 Hz, stereo, fltp, 128 kb/s
```

| Action                | Raspberry Pi 4 | Dell Inspiron 75650 | ? |
|-----------------------|----------------|---------------------|---|
| Read 10MiB            | 0.2 ± 0.0      |                     |   |
| Read 1GiB             | 24.4 ± 0.1     |                     |   |
| Transcode 30s 10MiB   | 32.1 ± 2.5     |                     |   |
| Transcode 30s 1GiB    | 8.6 ± 0.2      |                     |   |
| Transfer in LAN 10MiB | 2.7 ± 0.2      |                     |   |
| Transfer in LAN 1GiB  | 288.9 ± 2.0 s  |                     |   |
