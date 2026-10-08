pub mod decoder;
pub mod ffmpeg;
pub mod library;
pub mod output;
pub mod playback_state;
pub mod queue_manager;
pub mod replaygain;

pub use decoder::{decode_file, decode_track, decode_track_streaming, is_video_container, resample_and_remap_channels, DecodedTrack};
pub use library::{
    compute_library_stats, is_supported_audio_extension, read_track, sanitize_metadata_string,
    LibraryDatabase, LibraryFolder, LibraryStats, LibraryTrack, TagUpdate, TrackQuery,
};
pub use output::{
    generate_stereo_channel_test, generate_test_tone, get_audio_device_info, AudioDeviceInfo,
    AudioOutput, LatencyProfile, SampleQueue, SoundProfile, TargetLoudness,
};
pub use playback_state::{playback_state_channel, PlaybackClock, PlaybackStateReceiver, PlaybackStateSender, PlaybackUpdate, TrackMetadata, VISUALIZER_WINDOW_SAMPLES};
pub use queue_manager::GaplessQueueManager;
pub use replaygain::{analyze_and_tag, calculate_replaygain, ReplayGainEvent, ReplayGainResult, ReplayGainWorker, REPLAYGAIN_REFERENCE_LUFS};