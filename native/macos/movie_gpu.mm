#import "movie.hpp"
#include <cmath>

static const char *shader = R"(
#include <metal_stdlib>
using namespace metal;
struct Params { float4 inverseX; float4 inverseY; float4 geometry; float4 coefficients; };
kernel void convert_video(texture2d<float, access::sample> luma [[texture(0)]],
    texture2d<float, access::sample> chroma [[texture(1)]],
    texture2d<float, access::write> target [[texture(2)]],
    constant Params &p [[buffer(0)]], uint2 xy [[thread_position_in_grid]]) {
    if (xy.x >= target.get_width() || xy.y >= target.get_height()) return;
    float2 outSize = float2(target.get_width(), target.get_height());
    float scale = min(outSize.x / p.geometry.z, outSize.y / p.geometry.w);
    float2 display = (float2(xy) + 0.5 - (outSize - p.geometry.zw * scale) * 0.5) / scale;
    if (any(display < 0.0) || any(display >= p.geometry.zw)) { target.write(float4(0,0,0,1), xy); return; }
    display += p.geometry.xy;
    float2 source = float2(dot(p.inverseX.xyz, float3(display,1)), dot(p.inverseY.xyz, float3(display,1)));
    float2 uv = source / float2(luma.get_width(), luma.get_height());
    constexpr sampler s(coord::normalized, address::clamp_to_edge, filter::linear);
    float y = luma.sample(s, uv).r;
    float2 c = chroma.sample(s, uv).rg - float2(128.0/255.0);
    if (p.coefficients.z < 0.5) { y = (y - 16.0/255.0) * (255.0/219.0); c *= 255.0/224.0; }
    float kr = p.coefficients.x, kb = p.coefficients.y, kg = 1.0 - kr - kb;
    float3 rgb = float3(y + 2.0*(1.0-kr)*c.y,
        y - 2.0*kb*(1.0-kb)/kg*c.x - 2.0*kr*(1.0-kr)/kg*c.y,
        y + 2.0*(1.0-kb)*c.x);
    target.write(float4(clamp(rgb,0.0,1.0),1.0), xy);
})";

std::shared_ptr<MovieContext> movie_context(id<MTLDevice> device, id<MTLCommandQueue> queue, std::string &error) {
    auto result = std::make_shared<MovieContext>();
    result->device = device; result->queue = queue;
    NSError *failure = nil;
    id<MTLLibrary> library = [device newLibraryWithSource:[NSString stringWithUTF8String:shader] options:nil error:&failure];
    if (!library) { error = failure.localizedDescription.UTF8String ?: "Cannot compile native video conversion"; return {}; }
    result->pipeline = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"convert_video"] error:&failure];
    if (!result->pipeline) { error = failure.localizedDescription.UTF8String ?: "Cannot prepare video conversion"; return {}; }
    if (CVMetalTextureCacheCreate(kCFAllocatorDefault, nullptr, device, nullptr, &result->textures)) {
        error = "Cannot create native video texture cache"; return {};
    }
    return result;
}
bool movie_map(MoviePlayer &player, MovieFrame &frame, std::string &error) {
    OSType format = CVPixelBufferGetPixelFormatType(frame.image);
    if (format != kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange && format != kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) {
        error = "Hardware decoder did not supply a Metal-compatible NV12 frame"; return false;
    }
    if (!CVPixelBufferGetIOSurface(frame.image)) { error = "Video frame has no IOSurface"; return false; }
    for (unsigned plane = 0; plane < 2; plane++) {
        auto *texture = plane == 0 ? &frame.y : &frame.uv;
        if (CVMetalTextureCacheCreateTextureFromImage(kCFAllocatorDefault, player.context->textures, frame.image, nullptr,
            plane == 0 ? MTLPixelFormatR8Unorm : MTLPixelFormatRG8Unorm,
            CVPixelBufferGetWidthOfPlane(frame.image, plane), CVPixelBufferGetHeightOfPlane(frame.image, plane), plane, texture)) {
            error = "Cannot map decoded video planes to Metal"; return false;
        }
    }
    return true;
}
void movie_configure(MoviePlayer &player, CVPixelBufferRef image) {
    CGFloat width = CVPixelBufferGetWidth(image), height = CVPixelBufferGetHeight(image);
    CGAffineTransform transform = player.decoder->transform;
    CGRect display = CGRectApplyAffineTransform(CGRectMake(0,0,width,height), transform);
    CGAffineTransform inverse = CGAffineTransformInvert(transform);
    MovieUniforms params;
    params.inverseX = {(float)inverse.a,(float)inverse.c,(float)inverse.tx,0};
    params.inverseY = {(float)inverse.b,(float)inverse.d,(float)inverse.ty,0};
    params.geometry = {(float)display.origin.x,(float)display.origin.y,(float)display.size.width,(float)display.size.height};
    float kr = 0.2126, kb = 0.0722;
    CFTypeRef matrix = CVBufferGetAttachment(image, kCVImageBufferYCbCrMatrixKey, nullptr);
    if (matrix && CFEqual(matrix, kCVImageBufferYCbCrMatrix_ITU_R_601_4)) { kr = 0.299; kb = 0.114; }
    if (matrix && CFEqual(matrix, kCVImageBufferYCbCrMatrix_ITU_R_2020)) { kr = 0.2627; kb = 0.0593; }
    params.coefficients = {kr,kb,CVPixelBufferGetPixelFormatType(image) == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange ? 1.f : 0.f,0};
    player.uniforms = [player.context->device newBufferWithBytes:&params length:sizeof(params) options:MTLResourceStorageModeShared];
    player.configured = true;
}
