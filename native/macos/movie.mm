#import "movie.hpp"
#include <algorithm>

using Player = std::shared_ptr<MoviePlayer>;
using Context = std::shared_ptr<MovieContext>;
static void failure(const std::string &message, char *out, size_t capacity) {
    if (capacity) snprintf(out, capacity, "%s", message.c_str());
}
std::shared_ptr<MovieFrame> MoviePlayer::find(uint64_t key) {
    for (auto &frame : frames) if (frame && frame->key == key) return frame;
    return {};
}
bool MoviePlayer::decode(uint64_t key, std::string &error) {
    {
        std::lock_guard lock(mutex);
        auto cached = find(key);
        if (cached) { cached->used = ++age; cacheHits++; return true; }
    }
    CVPixelBufferRef image = decoder->decode((double)key / fps.load(), wanted, error);
    starts = decoder->starts; resets = decoder->resets;
    if (!image) return false;
    auto frame = std::make_shared<MovieFrame>();
    frame->image = image; frame->key = key; frame->seconds = decoder->position;
    frame->bytes = CVPixelBufferGetDataSize(image);
    if (!movie_map(*this, *frame, error)) return false;
    if (!configured) movie_configure(*this, image);
    std::lock_guard lock(mutex);
    size_t bytes = frame->bytes;
    for (auto &f : frames) if (f) bytes += f->bytes;
    // Bounded requested-frame LRU; GOP preroll frames are never retained here.
    while (bytes > 64 * 1024 * 1024) {
        auto oldest = std::min_element(frames.begin(), frames.end(), [](auto &a, auto &b) {
            return a && (!b || a->used < b->used);
        });
        if (oldest == frames.end() || !*oldest) break;
        bytes -= (*oldest)->bytes; oldest->reset();
    }
    auto slot = std::find(frames.begin(), frames.end(), nullptr);
    if (slot == frames.end()) slot = std::min_element(frames.begin(), frames.end(), [](auto &a, auto &b) { return a->used < b->used; });
    frame->used = ++age; *slot = frame;
    return true;
}
bool MoviePlayer::encode(uint64_t key, id<MTLSharedEvent> event, uint64_t value) {
    if (inFlight.load() >= 2 || !mutex.try_lock()) return false;
    auto frame = find(key); mutex.unlock();
    if (!frame) return false;
    id<MTLCommandBuffer> command = [context->queue commandBuffer];
    id<MTLComputeCommandEncoder> pass = [command computeCommandEncoder];
    [pass setComputePipelineState:context->pipeline];
    [pass setTexture:CVMetalTextureGetTexture(frame->y) atIndex:0];
    [pass setTexture:CVMetalTextureGetTexture(frame->uv) atIndex:1];
    [pass setTexture:texture atIndex:2]; [pass setBuffer:uniforms offset:0 atIndex:0];
    [pass dispatchThreads:MTLSizeMake(texture.width,texture.height,1) threadsPerThreadgroup:MTLSizeMake(16,16,1)];
    [pass endEncoding]; [command encodeSignalEvent:event value:value];
    inFlight++;
    auto owner = shared_from_this();
    [command addCompletedHandler:^(id<MTLCommandBuffer>) { (void)frame; owner->inFlight.fetch_sub(1); }];
    [command commit]; return true;
}
extern "C" void *vjay_movie_context_create(void *device, void *queue, char *error, size_t capacity) {
    @autoreleasepool {
        std::string message;
        auto result = movie_context((__bridge id<MTLDevice>)device, (__bridge id<MTLCommandQueue>)queue, message);
        if (!result) { failure(message,error,capacity); return nullptr; }
        return new Context(result);
    }
}
extern "C" void vjay_movie_context_destroy(void *context) { delete static_cast<Context *>(context); }
extern "C" void *vjay_movie_create(void *context, void *texture) {
    auto player = std::make_shared<MoviePlayer>();
    player->context = *static_cast<Context *>(context);
    player->texture = (__bridge id<MTLTexture>)texture;
    return new Player(player);
}
extern "C" bool vjay_movie_open(void *handle, const char *path, char *error, size_t capacity) {
    @autoreleasepool {
        auto &player = *static_cast<Player *>(handle);
        player->decoder = std::make_unique<MovieDecoder>();
        std::string message;
        if (!player->decoder->open([NSString stringWithUTF8String:path],message)) { failure(message,error,capacity); return false; }
        player->duration = player->decoder->duration; player->fps = player->decoder->fps;
        player->starts = player->decoder->starts;
        return true;
    }
}
extern "C" void vjay_movie_request(void *handle, uint64_t key) { (*static_cast<Player *>(handle))->wanted = key; }
extern "C" int vjay_movie_decode(void *handle, uint64_t key, char *error, size_t capacity) {
    @autoreleasepool {
        std::string message;
        bool result = (*static_cast<Player *>(handle))->decode(key,message);
        if (!message.empty()) { failure(message,error,capacity); return -1; }
        return result ? 1 : 0;
    }
}
extern "C" bool vjay_movie_encode(void *handle, uint64_t key, void *event, uint64_t value) {
    @autoreleasepool {
        auto player = *static_cast<Player *>(handle);
        return player->encode(key,(__bridge id<MTLSharedEvent>)event,value);
    }
}
extern "C" double vjay_movie_duration(void *handle) { return (*static_cast<Player *>(handle))->duration.load(); }
extern "C" double vjay_movie_fps(void *handle) { return (*static_cast<Player *>(handle))->fps.load(); }
extern "C" uint64_t vjay_movie_stat(void *handle, unsigned field) {
    auto &player = *static_cast<Player *>(handle);
    return field == 0 ? player->starts.load() : field == 1 ? player->resets.load() : player->cacheHits.load();
}
extern "C" void vjay_movie_destroy(void *handle) { @autoreleasepool { delete static_cast<Player *>(handle); } }
