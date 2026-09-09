use std::{sync::{atomic::{AtomicBool, Ordering}, Arc}, thread};

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use kanono_audio_engine::SampleQueue;

const FRAMES: usize = 256;
const CHANNELS: usize = 2;
const SAMPLES: usize = FRAMES * CHANNELS;

fn benchmark_audio_callback(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("audio_callback");
    group.throughput(Throughput::Elements(SAMPLES as u64));

    group.bench_function("stereo_256_frames", |bench| {
        let queue = SampleQueue::default();
        bench.iter_batched(
            || {
                queue.push_interleaved(std::iter::repeat_n(0.25_f32, SAMPLES));
                vec![0.0; SAMPLES]
            },
            |mut output| {
                queue.fill_output(black_box(&mut output));
                black_box(output);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("underrun_zero_fill", |bench| {
        let queue = SampleQueue::default();
        bench.iter(|| {
            let mut output = [1.0_f32; SAMPLES];
            queue.fill_output(black_box(&mut output));
            black_box(output);
        });
    });

    group.bench_function("concurrent_decode_producer", |bench| {
        let queue = Arc::new(SampleQueue::default());
        let producer_queue = Arc::clone(&queue);
        let running = Arc::new(AtomicBool::new(true));
        let producer_running = Arc::clone(&running);
        let producer = thread::spawn(move || loop {
            if !producer_running.load(Ordering::Relaxed) { break; }
            producer_queue.push_interleaved(std::iter::repeat_n(0.25_f32, SAMPLES));
        });
        bench.iter(|| {
            let mut output = [0.0_f32; SAMPLES];
            queue.fill_output(black_box(&mut output));
            black_box(output);
        });
        running.store(false, Ordering::Relaxed);
        producer.join().expect("benchmark producer panicked");
    });
    group.finish();
}

criterion_group!(benches, benchmark_audio_callback);
criterion_main!(benches);