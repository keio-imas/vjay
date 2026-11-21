#import "movie.hpp"
#include <cmath>
#include <algorithm>
#pragma clang diagnostic ignored "-Wdeprecated-declarations"

MovieDecoder::~MovieDecoder() {
    if (reader) [reader cancelReading];
    if (session) { VTDecompressionSessionWaitForAsynchronousFrames(session); VTDecompressionSessionInvalidate(session); CFRelease(session); }
    if (candidate) CFRelease(candidate);
    if (format) CFRelease(format);
}
void MovieDecoder::received(void *owner, void *, OSStatus status, VTDecodeInfoFlags,
    CVImageBufferRef image, CMTime pts, CMTime) {
    MovieDecoder *self = static_cast<MovieDecoder *>(owner);
    std::lock_guard lock(self->outputMutex);
    if (status) { self->callbackError = status; return; }
    if (image && CMTimeCompare(pts, self->target) == 0) {
        if (self->candidate) CFRelease(self->candidate);
        self->candidate = CVPixelBufferRetain(image);
    } else if (image && CMTimeCompare(pts, self->target) > 0) {
        // B-frame references can arrive before an earlier presentation frame.
        // Keep bounded lookahead instead of discarding already consumed samples.
        auto frame = std::make_shared<MovieFrame>();
        frame->image = CVPixelBufferRetain(image);
        frame->seconds = CMTimeGetSeconds(pts);
        frame->bytes = CVPixelBufferGetDataSize(image);
        auto slot = std::find(self->ahead.begin(), self->ahead.end(), nullptr);
        if (slot == self->ahead.end()) {
            slot = std::max_element(self->ahead.begin(), self->ahead.end(), [](auto &a, auto &b) { return a->seconds < b->seconds; });
            if ((*slot)->seconds < frame->seconds) return;
        }
        *slot = frame;
        size_t bytes = 0;
        for (auto &f : self->ahead) if (f) bytes += f->bytes;
        while (bytes > 64 * 1024 * 1024) {
            auto farthest = std::max_element(self->ahead.begin(), self->ahead.end(), [](auto &a, auto &b) {
                return b && (!a || a->seconds < b->seconds);
            });
            bytes -= (*farthest)->bytes; farthest->reset();
        }
    }
}
bool MovieDecoder::open(NSString *path, std::string &error) {
    asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:path]
        options:@{AVURLAssetPreferPreciseDurationAndTimingKey:@YES}];
    track = [asset tracksWithMediaType:AVMediaTypeVideo].firstObject;
    if (!track) { error = "File has no native video track"; return false; }
    duration = CMTimeGetSeconds(asset.duration); fps = track.nominalFrameRate;
    double interval = CMTimeGetSeconds(track.minFrameDuration);
    if (std::isfinite(interval) && interval > 0 && std::abs(1.0 / interval - fps) < 0.01)
        fps = 1.0 / interval;
    if (!std::isfinite(duration) || duration <= 0) { error = "Video has no finite duration"; return false; }
    if (!std::isfinite(fps) || fps <= 0) fps = 30;
    transform = track.preferredTransform;
    format = movie_format((__bridge CMVideoFormatDescriptionRef)track.formatDescriptions.firstObject, error);
    if (!format) { error = "Video has no format description"; return false; }
    CMVideoCodecType codec = CMFormatDescriptionGetMediaSubType(format);
    VTRegisterProfessionalVideoWorkflowVideoDecoders();
    VTRegisterSupplementalVideoDecoderIfAvailable(codec);
    NSDictionary *spec = @{(id)kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder:@YES};
    NSDictionary *attrs = @{
        (id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange),
        (id)kCVPixelBufferMetalCompatibilityKey:@YES,
        (id)kCVPixelBufferIOSurfacePropertiesKey:@{}
    };
    VTDecompressionOutputCallbackRecord callback = {received, this};
    OSStatus status = VTDecompressionSessionCreate(kCFAllocatorDefault, format,
        (__bridge CFDictionaryRef)spec, (__bridge CFDictionaryRef)attrs, &callback, &session);
    if (status) { error = "Cannot create required hardware decoder: " + std::to_string(status); return false; }
    CFTypeRef hardware = nullptr;
    status = VTSessionCopyProperty(session, kVTDecompressionPropertyKey_UsingHardwareAcceleratedVideoDecoder, kCFAllocatorDefault, &hardware);
    bool accelerated = !status && hardware && CFEqual(hardware, kCFBooleanTrue);
    if (hardware) CFRelease(hardware);
    if (!accelerated) { error = "Native movie decoder is not hardware accelerated"; return false; }
    starts++;
    VTSessionSetProperty(session, kVTDecompressionPropertyKey_RealTime, kCFBooleanTrue);
    return true;
}
bool MovieDecoder::reset(AVSampleCursor *cursor, std::string &error) {
    AVSampleCursor *sync = [cursor copy];
    unsigned count = 0;
    while (!sync.currentSampleSyncInfo.sampleIsFullSync && [sync stepInDecodeOrderByCount:-1] && ++count < 24000) {}
    CMTime start = sync.presentationTimeStamp;
    if (!CMTIME_IS_NUMERIC(start)) start = kCMTimeZero;
    if (CMTimeCompare(start, kCMTimeZero) < 0) start = kCMTimeZero;
    if (reader) { [reader cancelReading]; resets++; }
    for (auto &frame : ahead) frame.reset();
    decodedThrough = kCMTimeInvalid;
    NSError *failure = nil;
    reader = [[AVAssetReader alloc] initWithAsset:asset error:&failure];
    if (!reader) { error = failure.localizedDescription.UTF8String ?: "Cannot open native sample reader"; return false; }
    reader.timeRange = CMTimeRangeMake(start, kCMTimePositiveInfinity);
    output = [[AVAssetReaderTrackOutput alloc] initWithTrack:track outputSettings:nil];
    output.alwaysCopiesSampleData = NO;
    if (![reader canAddOutput:output]) { error = "Cannot read compressed video samples"; return false; }
    [reader addOutput:output];
    if (![reader startReading]) { error = reader.error.localizedDescription.UTF8String ?: "Cannot start native sample reader"; return false; }
    return true;
}
CVPixelBufferRef MovieDecoder::decode(double seconds,
    const std::atomic<uint64_t> &wanted, std::string &error) {
    if (candidate) { CFRelease(candidate); candidate = nullptr; }
    callbackError = noErr;
    AVSampleCursor *cursor = [track makeSampleCursorWithPresentationTimeStamp:CMTimeMakeWithSeconds(seconds + 1e-7, 600000)];
    if (!cursor) { error = "Cannot locate the requested video sample"; return nullptr; }
    target = cursor.presentationTimeStamp;
    double actual = CMTimeGetSeconds(target);
    for (auto &frame : ahead) {
        if (frame && std::abs(frame->seconds - actual) < 1e-7) {
            position = actual;
            CVPixelBufferRef result = CVPixelBufferRetain(frame->image);
            frame.reset(); return result;
        }
    }
    bool consumed = CMTIME_IS_NUMERIC(decodedThrough)
        && CMTimeCompare(cursor.decodeTimeStamp, decodedThrough) <= 0;
    if (!reader || consumed || actual < position - 1e-7 || actual - position > 2.0 || reader.status != AVAssetReaderStatusReading) {
        if (!reset(cursor, error)) return nullptr;
    }
    // Finish an in-progress target while newer requests are coalesced by the
    // worker. Cancelling on every scrub update can starve a long-GOP decoder.
    for (unsigned count = 0; count < 24000 && wanted.load() != UINT64_MAX; count++) {
        CMSampleBufferRef sample = [output copyNextSampleBuffer];
        if (!sample) {
            VTDecompressionSessionFinishDelayedFrames(session);
            VTDecompressionSessionWaitForAsynchronousFrames(session);
            if (!candidate) error = reader.error.localizedDescription.UTF8String ?: "Requested video sample was not decoded";
            break;
        }
        // AVAssetReader may emit empty edit/timeline marker buffers before data.
        // They are not compressed pictures and must never reach VideoToolbox.
        if (!CMSampleBufferGetNumSamples(sample) || !CMSampleBufferGetFormatDescription(sample)
            || !CMSampleBufferGetTotalSampleSize(sample)) { CFRelease(sample); continue; }
        sample = movie_sample(sample, format, error);
        if (!sample) break;
        bool requested = CMTimeCompare(CMSampleBufferGetPresentationTimeStamp(sample), target) == 0;
        decodedThrough = CMSampleBufferGetDecodeTimeStamp(sample);
        if (!CMTIME_IS_NUMERIC(decodedThrough)) decodedThrough = CMSampleBufferGetPresentationTimeStamp(sample);
        VTDecodeFrameFlags flags = kVTDecodeFrame_EnableAsynchronousDecompression;
        if (CMTimeCompare(CMSampleBufferGetPresentationTimeStamp(sample), target) < 0)
            flags |= kVTDecodeFrame_DoNotOutputFrame;
        OSStatus status = VTDecompressionSessionDecodeFrame(session, sample, flags, nullptr, nullptr);
        CFRelease(sample);
        if (status) { error = "Native video decode failed: " + std::to_string(status); break; }
        // Batch reference-only GOP preroll. Only requested pictures need output
        // surfaces; one wait per packet would serialize hardware decode.
        if (requested || count % 16 == 15) {
            VTDecompressionSessionWaitForAsynchronousFrames(session);
            if (callbackError) { error = "Native video callback failed: " + std::to_string(callbackError); break; }
            if (candidate) break;
        }
    }
    VTDecompressionSessionWaitForAsynchronousFrames(session);
    if (candidate && error.empty() && wanted.load() != UINT64_MAX) {
        position = actual;
        CVPixelBufferRef result = candidate; candidate = nullptr; return result;
    }
    return nullptr;
}
