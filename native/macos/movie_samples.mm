#import "movie.hpp"

CMVideoFormatDescriptionRef movie_format(CMVideoFormatDescriptionRef source, std::string &error) {
    if (!source) { error = "Video has no format description"; return nullptr; }
    // AVFoundation exposes the MP4 hev1 sample-entry tag. VideoToolbox expects
    // hvc1; retain the same hvcC parameter sets and timing without recoding.
    if (CMFormatDescriptionGetMediaSubType(source) != 'hev1') {
        CFRetain(source); return source;
    }
    CMVideoDimensions size = CMVideoFormatDescriptionGetDimensions(source);
    CMVideoFormatDescriptionRef result = nullptr;
    OSStatus status = CMVideoFormatDescriptionCreate(kCFAllocatorDefault,
        kCMVideoCodecType_HEVC, size.width, size.height,
        CMFormatDescriptionGetExtensions(source), &result);
    if (status) error = "Cannot normalize HEVC format: " + std::to_string(status);
    return result;
}
CMSampleBufferRef movie_sample(CMSampleBufferRef sample, CMVideoFormatDescriptionRef format, std::string &error) {
    if (CMFormatDescriptionGetMediaSubType(CMSampleBufferGetFormatDescription(sample))
        == CMFormatDescriptionGetMediaSubType(format)) return sample;
    CMSampleBufferRef result = nullptr;
    CMSampleTimingInfo timing;
    OSStatus status = CMSampleBufferGetSampleTimingInfo(sample, 0, &timing);
    size_t size = CMSampleBufferGetTotalSampleSize(sample);
    if (!status && CMSampleBufferGetNumSamples(sample) == 1) {
        status = CMSampleBufferCreateReady(kCFAllocatorDefault, CMSampleBufferGetDataBuffer(sample),
            format, 1, 1, &timing, 1, &size, &result);
    }
    if (result) {
        CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sample, false);
        if (attachments && CFArrayGetCount(attachments)) {
            CFArrayRef destination = CMSampleBufferGetSampleAttachmentsArray(result, true);
            NSMutableDictionary *dictionary = (__bridge NSMutableDictionary *)CFArrayGetValueAtIndex(destination, 0);
            [dictionary addEntriesFromDictionary:(__bridge NSDictionary *)CFArrayGetValueAtIndex(attachments, 0)];
        }
        CFDictionaryRef properties = CMCopyDictionaryOfAttachments(kCFAllocatorDefault, sample, kCMAttachmentMode_ShouldPropagate);
        if (properties) { CMSetAttachments(result, properties, kCMAttachmentMode_ShouldPropagate); CFRelease(properties); }
    } else error = "Cannot normalize HEVC sample: " + std::to_string(status);
    CFRelease(sample);
    return result;
}
