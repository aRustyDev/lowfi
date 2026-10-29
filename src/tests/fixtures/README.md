# Test tone fixture

`test-tone.mp3` is a checked-in, deterministic four-second 440 Hz sine wave.
It is mono, 44.1 kHz, and encoded as MP3 at 64 kb/s. The decoder test accepts
a reported duration from three to five seconds to allow MP3 encoder padding.

Regenerate it with FFmpeg from the repository root:

```sh
ffmpeg -f lavfi -i 'sine=frequency=440:sample_rate=44100:duration=4' \
  -ac 1 -c:a libmp3lame -b:a 64k -write_xing 1 \
  -y src/tests/fixtures/test-tone.mp3
```
