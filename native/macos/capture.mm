#import <AVFoundation/AVFoundation.h>
#import <CoreVideo/CoreVideo.h>
#import <Metal/Metal.h>
#import <MetalPerformanceShaders/MetalPerformanceShaders.h>
#include <atomic>
#include <mutex>

static NSArray<AVCaptureDevice *> *devices() {
    NSMutableArray *types = [NSMutableArray arrayWithObject:AVCaptureDeviceTypeBuiltInWideAngleCamera];
    #pragma clang diagnostic push
    #pragma clang diagnostic ignored "-Wunguarded-availability-new"
    if ([NSProcessInfo.processInfo isOperatingSystemAtLeastVersion:NSOperatingSystemVersion{14,0,0}]) [types addObject:AVCaptureDeviceTypeExternal];
    else {
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        [types addObject:AVCaptureDeviceTypeExternalUnknown];
#pragma clang diagnostic pop
    }
    #pragma clang diagnostic pop
    return [AVCaptureDeviceDiscoverySession discoverySessionWithDeviceTypes:types mediaType:AVMediaTypeVideo position:AVCaptureDevicePositionUnspecified].devices;
}
static void copy_text(NSString *s, char *out, size_t capacity) {
    if (capacity) snprintf(out, capacity, "%s", s.UTF8String ?: "");
}
extern "C" int vjay_capture_device_count() { @autoreleasepool { return (int)devices().count; } }
extern "C" bool vjay_capture_device_info(int index, char *uid, char *name, size_t capacity) {
    @autoreleasepool {
        NSArray *all = devices(); if (index < 0 || (NSUInteger)index >= all.count) return false;
        AVCaptureDevice *d = all[index]; copy_text(d.uniqueID, uid, capacity); copy_text(d.localizedName, name, capacity); return true;
    }
}

@interface VjayCapture : NSObject <AVCaptureVideoDataOutputSampleBufferDelegate> {
@public
    std::atomic<int> status;
    std::atomic<bool> stopped;
    std::atomic<unsigned> inFlight;
    std::mutex mutex;
    CVPixelBufferRef latest;
    CVMetalTextureCacheRef cache;
    id<MTLDevice> metal;
    id<MTLCommandQueue> queue;
    id<MTLTexture> target;
    MPSImageBilinearScale *scale;
    AVCaptureSession *session;
    AVCaptureVideoDataOutput *output;
    dispatch_queue_t control;
    dispatch_queue_t samples;
    id errorObserver;
}
- (void)start:(NSString *)uid width:(int)width height:(int)height fps:(int)fps;
- (void)stop;
- (BOOL)encode:(id<MTLSharedEvent>)event value:(uint64_t)value;
@end
@implementation VjayCapture
- (instancetype)init {
    if ((self=[super init])) {
        status=0; stopped=false; inFlight=0; latest=nullptr; cache=nullptr;
        control=dispatch_queue_create("vjay.camera.control", DISPATCH_QUEUE_SERIAL);
        samples=dispatch_queue_create("vjay.camera.samples", DISPATCH_QUEUE_SERIAL);
    } return self;
}
- (void)startAuthorized:(NSString *)uid width:(int)width height:(int)height fps:(int)fps {
    if (stopped.load()) return;
    @try {
        AVCaptureDevice *device=nil;
        for (AVCaptureDevice *d in devices()) if ([d.uniqueID isEqualToString:uid]) { device=d; break; }
        if (!device) { status=2; return; }
        AVCaptureDeviceFormat *format=nil;
        for (AVCaptureDeviceFormat *f in device.formats) {
            CMVideoDimensions dim=CMVideoFormatDescriptionGetDimensions(f.formatDescription);
            if (dim.width!=width || dim.height!=height) continue;
            for (AVFrameRateRange *range in f.videoSupportedFrameRateRanges)
                if (range.minFrameRate<=fps && range.maxFrameRate>=fps) { format=f; break; }
            if (format) break;
        }
        if (!format) { status=4; return; }
        NSError *error=nil;
        AVCaptureDeviceInput *input=[AVCaptureDeviceInput deviceInputWithDevice:device error:&error];
        if (!input) { status=5; return; }
        session=[AVCaptureSession new];
        [session beginConfiguration];
        if (![session canAddInput:input]) { [session commitConfiguration]; status=5; return; }
        [session addInput:input];
        AVCaptureSessionPreset preset=width>=3840 ? AVCaptureSessionPreset3840x2160 : (width>=1920 ? AVCaptureSessionPreset1920x1080 : (width>=1280 ? AVCaptureSessionPreset1280x720 : AVCaptureSessionPreset640x480));
        if (![session canSetSessionPreset:preset]) { [session commitConfiguration]; status=4; return; }
        session.sessionPreset=preset;
        if (![device lockForConfiguration:&error]) { [session commitConfiguration]; status=5; return; }
        @try {
            device.activeFormat=format;
            device.activeVideoMinFrameDuration=CMTimeMake(1,fps);
            device.activeVideoMaxFrameDuration=CMTimeMake(1,fps);
        } @finally { [device unlockForConfiguration]; }
        output=[AVCaptureVideoDataOutput new];
        output.alwaysDiscardsLateVideoFrames=YES;
        output.videoSettings=@{(id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32BGRA),
            (id)kCVPixelBufferMetalCompatibilityKey:@YES, (id)kCVPixelBufferIOSurfacePropertiesKey:@{}};
        [output setSampleBufferDelegate:self queue:samples];
        if (![session canAddOutput:output]) { [session commitConfiguration]; status=5; return; }
        [session addOutput:output]; [session commitConfiguration];
        __weak VjayCapture *weak=self;
        errorObserver=[[NSNotificationCenter defaultCenter] addObserverForName:AVCaptureSessionRuntimeErrorNotification object:session queue:nil usingBlock:^(NSNotification *) {
            VjayCapture *owner=weak; if (owner && !owner->stopped.load()) owner->status=9;
        }];
        if (!stopped.load()) { status=6; [session startRunning]; }
        if (stopped.load()) [session stopRunning];
    } @catch (NSException *) { status=5; }
}
- (void)start:(NSString *)uid width:(int)width height:(int)height fps:(int)fps {
    AVAuthorizationStatus authorization=[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];
    void (^begin)(void)=^{ dispatch_async(self->control, ^{ [self startAuthorized:uid width:width height:height fps:fps]; }); };
    if (authorization==AVAuthorizationStatusNotDetermined) {
        status=1;
        [AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo completionHandler:^(BOOL granted) {
            if (granted) begin(); else self->status=3;
        }];
    } else if (authorization==AVAuthorizationStatusAuthorized) begin();
    else status=3;
}
- (void)captureOutput:(AVCaptureOutput *)unused didOutputSampleBuffer:(CMSampleBufferRef)sample fromConnection:(AVCaptureConnection *)connection {
    if (stopped.load()) return;
    CVPixelBufferRef image=CMSampleBufferGetImageBuffer(sample);
    if (!image || CVPixelBufferGetPixelFormatType(image)!=kCVPixelFormatType_32BGRA) return;
    if (mutex.try_lock()) {
        CFRetain(image); CVPixelBufferRef old=latest; latest=image; mutex.unlock();
        if (old) CFRelease(old); status=7;
    }
}
- (BOOL)encode:(id<MTLSharedEvent>)event value:(uint64_t)value {
    if (inFlight.load()>=2 || !mutex.try_lock()) return NO;
    CVPixelBufferRef frame=latest; latest=nullptr; mutex.unlock();
    if (!frame) return NO;
    CVMetalTextureRef mapped=nullptr;
    CVReturn result=CVMetalTextureCacheCreateTextureFromImage(kCFAllocatorDefault,cache,frame,nullptr,MTLPixelFormatBGRA8Unorm,
        CVPixelBufferGetWidth(frame),CVPixelBufferGetHeight(frame),0,&mapped);
    if (result!=kCVReturnSuccess || !mapped) { CFRelease(frame); status=9; return NO; }
    id<MTLTexture> source=CVMetalTextureGetTexture(mapped);
    id<MTLCommandBuffer> command=[queue commandBuffer];
    inFlight.fetch_add(1);
    [scale encodeToCommandBuffer:command sourceTexture:source destinationTexture:target];
    [command encodeSignalEvent:event value:value];
    [command addCompletedHandler:^(id<MTLCommandBuffer>) {
        CFRelease(frame); CFRelease(mapped); self->inFlight.fetch_sub(1);
    }];
    [command commit]; return YES;
}
- (void)stop {
    stopped=true;
    dispatch_async(control, ^{
        @try { [self->output setSampleBufferDelegate:nil queue:nullptr]; [self->session stopRunning]; } @catch (NSException *) {}
        dispatch_sync(self->samples, ^{});
        if (self->errorObserver) [[NSNotificationCenter defaultCenter] removeObserver:self->errorObserver];
        self->errorObserver=nil;
    });
}
- (void)dealloc {
    if (latest) CFRelease(latest); if (cache) CFRelease(cache);
}
@end
extern "C" void *vjay_capture_create(void *device,void *queue,void *target,const char *uid,int width,int height,int fps,bool synthetic) {
    @autoreleasepool {
        VjayCapture *capture=[VjayCapture new];
        capture->metal=(__bridge id<MTLDevice>)device; capture->queue=(__bridge id<MTLCommandQueue>)queue;
        capture->target=(__bridge id<MTLTexture>)target;
        capture->scale=[[MPSImageBilinearScale alloc] initWithDevice:capture->metal];
        if (CVMetalTextureCacheCreate(kCFAllocatorDefault,nullptr,capture->metal,nullptr,&capture->cache)!=kCVReturnSuccess) return nullptr;
        if (!synthetic) [capture start:[NSString stringWithUTF8String:uid] width:width height:height fps:fps];
        else {
            CVPixelBufferRef buffer=nullptr;
            NSDictionary *attrs=@{(id)kCVPixelBufferMetalCompatibilityKey:@YES,(id)kCVPixelBufferIOSurfacePropertiesKey:@{}};
            CVPixelBufferCreate(kCFAllocatorDefault,128,72,kCVPixelFormatType_32BGRA,(__bridge CFDictionaryRef)attrs,&buffer);
            if (!buffer) return nullptr;
            CVMetalTextureRef mapped=nullptr;
            NSDictionary *usage=@{(id)kCVMetalTextureUsage:@(MTLTextureUsageShaderRead|MTLTextureUsageRenderTarget)};
            CVMetalTextureCacheCreateTextureFromImage(kCFAllocatorDefault,capture->cache,buffer,(__bridge CFDictionaryRef)usage,MTLPixelFormatBGRA8Unorm,128,72,0,&mapped);
            if (!mapped) {CFRelease(buffer); return nullptr;}
            MTLRenderPassDescriptor *pass=[MTLRenderPassDescriptor renderPassDescriptor];
            pass.colorAttachments[0].texture=CVMetalTextureGetTexture(mapped);
            pass.colorAttachments[0].loadAction=MTLLoadActionClear;
            pass.colorAttachments[0].storeAction=MTLStoreActionStore;
            pass.colorAttachments[0].clearColor=MTLClearColorMake(1,0,0,1);
            id<MTLCommandBuffer> cmd=[capture->queue commandBuffer];
            [[cmd renderCommandEncoderWithDescriptor:pass] endEncoding]; [cmd commit];
            capture->latest=buffer; capture->status=7; CFRelease(mapped);
        }
        return (__bridge_retained void *)capture;
    }
}
extern "C" bool vjay_capture_encode(void *handle,void *event,uint64_t value) {
    @autoreleasepool { return [(__bridge VjayCapture *)handle encode:(__bridge id<MTLSharedEvent>)event value:value]; }
}
extern "C" int vjay_capture_status(void *handle) { return ((__bridge VjayCapture *)handle)->status.load(); }
extern "C" void vjay_capture_destroy(void *handle) {
    @autoreleasepool { VjayCapture *capture=(__bridge_transfer VjayCapture *)handle; [capture stop]; }
}
