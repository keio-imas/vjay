use super::*;
use crate::audio::pcm::FRAMES;

fn fixture(rate: f32) -> (SourceConsumer, rtrb::Producer<Chunk>, Arc<Shared>) {
    let (producer, chunks) = rtrb::RingBuffer::new(2);
    let (recycle, _available) = rtrb::RingBuffer::new(4);
    let shared = Arc::new(Shared::default());
    shared.gain.store(1.0f32.to_bits(), Ordering::Release);
    shared.rate.store(rate.to_bits(), Ordering::Release);
    (
        SourceConsumer::new(Playback { chunks, recycle }, shared.clone(), 48000),
        producer,
        shared,
    )
}
fn chunk(epoch: u64, start: f64, value: f32) -> Chunk {
    Chunk {
        epoch,
        start,
        frames: vec![[value; 2]; FRAMES].into_boxed_slice(),
    }
}
fn push(producer: &mut rtrb::Producer<Chunk>, chunk: Chunk) {
    assert!(producer.push(chunk).is_ok());
}

#[test]
fn pending_seek_keeps_current_audio_and_switches_only_after_replacement_is_ready() {
    let (mut consumer, mut producer, shared) = fixture(1.0);
    push(&mut producer, chunk(1, 0.0, 0.25));
    consumer.begin_buffer();
    assert_eq!(consumer.frame(), [0.25; 2]);
    consumer.finish_buffer();
    for epoch in 2..70 {
        shared.epoch.store(epoch, Ordering::Release);
        consumer.begin_buffer();
        for _ in 0..128 {
            assert_eq!(consumer.frame(), [0.25; 2]);
        }
        consumer.finish_buffer();
    }
    push(&mut producer, chunk(12, 3.0, 0.75));
    consumer.begin_buffer();
    for _ in 0..64 {
        consumer.frame();
    }
    assert_eq!(consumer.frame(), [0.75; 2]);
    consumer.finish_buffer();
    assert_eq!(
        shared.active_epoch.load(Ordering::Acquire),
        12,
        "a completed request must play even when newer seeks are pending"
    );
    shared.rate.store(0.0f32.to_bits(), Ordering::Release);
    let consumed = shared.consumed.load(Ordering::Acquire);
    consumer.begin_buffer();
    for _ in 0..256 {
        assert_eq!(consumer.frame(), [0.0; 2]);
    }
    consumer.finish_buffer();
    assert_eq!(shared.consumed.load(Ordering::Acquire), consumed);
}
#[test]
fn fractional_resampling_interpolates_across_contiguous_pcm_windows() {
    let (mut consumer, mut producer, _) = fixture(0.5);
    push(&mut producer, chunk(1, 0.0, 0.25));
    push(&mut producer, chunk(1, FRAMES as f64 / 48000.0, 0.5));
    consumer.begin_buffer();
    for _ in 0..FRAMES * 2 - 1 {
        consumer.frame();
    }
    assert_eq!(consumer.frame(), [0.375; 2]);
    assert_eq!(consumer.frame(), [0.5; 2]);
}

#[test]
fn reported_playhead_wraps_inside_audio_range_and_clamps_without_loop() {
    for looping in [true, false] {
        let (mut consumer, mut producer, shared) = fixture(1.0);
        shared
            .range_start
            .store(1.0f64.to_bits(), Ordering::Release);
        shared.range_end.store(2.0f64.to_bits(), Ordering::Release);
        shared.looping.store(looping, Ordering::Release);
        push(&mut producer, chunk(1, 1.99, 0.25));
        consumer.begin_buffer();
        for _ in 0..960 {
            consumer.frame();
        }
        consumer.finish_buffer();
        let position = f64::from_bits(shared.playhead.load(Ordering::Acquire));
        assert!(
            (position - if looping { 1.01 } else { 2.0 }).abs() < 1e-8,
            "{position}"
        );
    }
}
