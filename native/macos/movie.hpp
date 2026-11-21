#pragma once
#import <AVFoundation/AVFoundation.h>
#import <CoreVideo/CoreVideo.h>
#import <Metal/Metal.h>
#import <VideoToolbox/VideoToolbox.h>
#import <simd/simd.h>
#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <string>

struct MovieContext {
    id<MTLDevice> device;
    id<MTLCommandQueue> queue;
    id<MTLComputePipelineState> pipeline;
    CVMetalTextureCacheRef textures = nullptr;
    ~MovieContext() { if (textures) CFRelease(textures); }
};
struct MovieFrame {
    CVPixelBufferRef image = nullptr;
    CVMetalTextureRef y = nullptr, uv = nullptr;
    uint64_t key = 0, used = 0;
    double seconds = 0;
    size_t bytes = 0;
    ~MovieFrame() { if (y) CFRelease(y); if (uv) CFRelease(uv); if (image) CFRelease(image); }
};
struct MovieUniforms {
    simd_float4 inverseX, inverseY;
    simd_float4 geometry; // display origin xy, display size zw
    simd_float4 coefficients; // Kr, Kb, full range, unused
};
struct MovieDecoder {
    AVURLAsset *asset = nil;
    AVAssetTrack *track = nil;
    AVAssetReader *reader = nil;
    AVAssetReaderTrackOutput *output = nil;
    VTDecompressionSessionRef session = nullptr;
    CMVideoFormatDescriptionRef format = nullptr;
    CVPixelBufferRef candidate = nullptr;
    std::array<std::shared_ptr<MovieFrame>, 4> ahead;
    std::mutex outputMutex;
    CMTime target = kCMTimeZero;
    CMTime decodedThrough = kCMTimeInvalid;
    double duration = 0, fps = 30, position = -1;
    CGAffineTransform transform = CGAffineTransformIdentity;
    OSStatus callbackError = noErr;
    uint64_t starts = 0, resets = 0;
    bool open(NSString *path, std::string &error);
    CVPixelBufferRef decode(double seconds, const std::atomic<uint64_t> &wanted, std::string &error);
    bool reset(AVSampleCursor *cursor, std::string &error);
    static void received(void *, void *, OSStatus, VTDecodeInfoFlags, CVImageBufferRef, CMTime, CMTime);
    ~MovieDecoder();
};
struct MoviePlayer : std::enable_shared_from_this<MoviePlayer> {
    std::shared_ptr<MovieContext> context;
    std::unique_ptr<MovieDecoder> decoder;
    id<MTLTexture> texture;
    id<MTLBuffer> uniforms;
    std::array<std::shared_ptr<MovieFrame>, 4> frames;
    std::mutex mutex;
    std::atomic<uint64_t> wanted{0}, cacheHits{0}, starts{0}, resets{0};
    std::atomic<unsigned> inFlight{0};
    std::atomic<double> duration{0}, fps{30};
    uint64_t age = 0;
    bool configured = false;
    std::shared_ptr<MovieFrame> find(uint64_t key);
    bool decode(uint64_t key, std::string &error);
    bool encode(uint64_t key, id<MTLSharedEvent> event, uint64_t value);
};
std::shared_ptr<MovieContext> movie_context(id<MTLDevice>, id<MTLCommandQueue>, std::string &);
bool movie_map(MoviePlayer &, MovieFrame &, std::string &);
void movie_configure(MoviePlayer &, CVPixelBufferRef);
CMVideoFormatDescriptionRef movie_format(CMVideoFormatDescriptionRef, std::string &);
CMSampleBufferRef movie_sample(CMSampleBufferRef, CMVideoFormatDescriptionRef, std::string &);
